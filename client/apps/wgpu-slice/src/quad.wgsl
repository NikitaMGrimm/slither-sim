struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) alpha: vec2<f32>,
    @location(2) @interpolate(flat) layer: u32,
};
@vertex fn vs(@location(0) pos: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) alpha: vec2<f32>, @location(3) layer: u32) -> VertexOut {
    var out: VertexOut;
    out.position = vec4<f32>(pos, 0., 1.);
    out.uv = uv;
    out.alpha = alpha;
    out.layer = layer;
    return out;
}
@group(0) @binding(0) var source: texture_2d_array<f32>;
@group(0) @binding(1) var source_sampler: sampler;
@fragment fn fs(v: VertexOut) -> @location(0) vec4<f32> {
    return textureSample(source, source_sampler, v.uv, i32(v.layer)) * v.alpha.x;
}

@fragment fn fs_black(v: VertexOut) -> @location(0) vec4<f32> {
    let alpha = textureSample(source, source_sampler, v.uv, i32(v.layer)).a * v.alpha.x;
    if alpha == 0. { return vec4<f32>(0.); }
    // The Canvas reference truncates each black layer on the 8-bit destination.
    // Reverse-subtract a near-half-byte bias to counter hardware nearest rounding.
    // The 255/256 factor matches its integer attenuation on dark backgrounds;
    // this is not a general replacement for colored-image source-over.
    // Integer-alpha Vulkan/MSAA4 probe: exact for destination bytes 0..18;
    // GPU blend precision leaves at most one byte of error elsewhere.
    let bias = (0.5 - 1. / 512.) / 255.;
    return vec4<f32>(vec3<f32>(bias), alpha * (255. / 256.));
}

@vertex fn vs_composite(@builtin(vertex_index) i: u32) -> VertexOut {
    let p = array<vec2<f32>,3>(vec2<f32>(-1.,-1.),vec2<f32>(3.,-1.),vec2<f32>(-1.,3.));
    var out: VertexOut; out.position=vec4<f32>(p[i],0.,1.);
    out.uv=vec2<f32>((p[i].x+1.)/2.,(1.-p[i].y)/2.);out.alpha=vec2<f32>(1.,0.);out.layer=0u;return out;
}

// Catmull-Rom weights keep broad glow gradients sharper than bilinear upscaling.
fn cubic(t: f32) -> vec4<f32> {
    let t2=t*t; let t3=t2*t;
    return vec4<f32>(-.5*t+t2-.5*t3, 1.-2.5*t2+1.5*t3, .5*t+2.*t2-1.5*t3, -.5*t2+.5*t3);
}
@fragment fn fs_glow_composite(v: VertexOut) -> @location(0) vec4<f32> {
    let size=vec2<f32>(textureDimensions(source,0));
    let p=v.uv*size-.5; let base=floor(p); let f=fract(p);
    let wx=cubic(f.x); let wy=cubic(f.y);
    let x=vec3<f32>(base.x-1.,base.x+wx.z/(wx.y+wx.z),base.x+2.);
    let y=vec3<f32>(base.y-1.,base.y+wy.z/(wy.y+wy.z),base.y+2.);
    let ax=vec3<f32>(wx.x,wx.y+wx.z,wx.w);let ay=vec3<f32>(wy.x,wy.y+wy.z,wy.w);
    let center=textureSampleLevel(source,source_sampler,(vec2<f32>(x[1],y[1])+.5)/size,0,0.);
    let left=textureSampleLevel(source,source_sampler,(vec2<f32>(x[0],y[1])+.5)/size,0,0.);
    let right=textureSampleLevel(source,source_sampler,(vec2<f32>(x[2],y[1])+.5)/size,0,0.);
    let top=textureSampleLevel(source,source_sampler,(vec2<f32>(x[1],y[0])+.5)/size,0,0.);
    let bottom=textureSampleLevel(source,source_sampler,(vec2<f32>(x[1],y[2])+.5)/size,0,0.);
    // Fold the four small corner weights into the centre: five filtered
    // samples, unit DC gain. Isolated/readback budgets live in sprite_layers.rs.
    let color=center*(ax[1]*ay[1]+(ax[0]+ax[2])*(ay[0]+ay[2]))
        +left*ax[0]*ay[1]+right*ax[2]*ay[1]+top*ax[1]*ay[0]+bottom*ax[1]*ay[2];
    return clamp(color,vec4<f32>(0.),vec4<f32>(1.));
}

// Retain each original 8-bit contribution. Summing opacities before rounding
// would turn two individually invisible contributions into visible light.
@fragment fn fs_pair(v: VertexOut) -> @location(0) vec4<f32> {
    let color=textureSample(source,source_sampler,v.uv,i32(v.layer));
    let a=round(color*v.alpha.x*255.)/255.;
    let b=round(color*v.alpha.y*255.)/255.;
    return a+b;
}
