// Exact projected pellet pairs use the unchanged shared image fragments.
// The CPU core owns animation, source rectangles, centres and alpha. WGSL only
// selects packed endpoints and applies UV crop; this vertex-input expansion must
// live in WGSL, while the shared quad.wgsl fragments own paired-addition rounding.
struct PelletView { rules: vec4<u32> };
struct PelletSource { selection: vec4<u32> };
@group(1) @binding(0) var<uniform> pellet_view: PelletView;
@group(2) @binding(0) var<uniform> pellet_source: PelletSource;
struct PelletIn {
    @location(0) body_rect: vec4<f32>,
    @location(1) inner_rect: vec4<f32>,
    @location(2) outer_rect: vec4<f32>,
    @location(3) alpha0: vec4<f32>,
    @location(4) alpha1_uv0: vec4<f32>,
    @location(5) uv12: vec4<f32>,
    @location(6) flags: u32,
};
fn pellet_vertex(v: PelletIn, vertex: u32, requested_bin: u32) -> VertexOut {
    var out: VertexOut;
    out.position = vec4<f32>(2., 2., 0., 1.);
    out.uv = vec2<f32>(0.);
    out.alpha = vec2<f32>(0.);
    out.layer = pellet_source.selection.x;
    let part = pellet_source.selection.y;
    if (v.flags & 1u) == 0u || (v.flags & (32u << part)) != 0u { return out; }
    if part != 0u {
        let shift = select(1u, 3u, part == 2u);
        let bin = (v.flags >> shift) & 3u;
        let reduced = (pellet_view.rules.x & (1u << bin)) != 0u;
        if select(0u, bin, reduced) != requested_bin { return out; }
    }
    let corners = array<vec2<f32>, 4>(
        vec2<f32>(0., 0.), vec2<f32>(1., 0.), vec2<f32>(1., 1.), vec2<f32>(0., 1.)
    );
    let uv = corners[vertex];
    var rect = v.body_rect;
    var alpha = v.alpha0.xy;
    var crop = v.alpha1_uv0.zw;
    if part == 1u { rect = v.inner_rect; alpha = v.alpha0.zw; crop = v.uv12.xy; }
    if part == 2u { rect = v.outer_rect; alpha = v.alpha1_uv0.xy; crop = v.uv12.zw; }
    out.position = vec4<f32>(select(rect.x, rect.z, uv.x == 1.), select(rect.y, rect.w, uv.y == 1.), 0., 1.);
    out.uv = uv * crop;
    out.alpha = alpha;
    return out;
}
@vertex fn vs_pellet_body(v: PelletIn, @builtin(vertex_index) i: u32) -> VertexOut { return pellet_vertex(v, i, 4u); }
@vertex fn vs_pellet_full(v: PelletIn, @builtin(vertex_index) i: u32) -> VertexOut { return pellet_vertex(v, i, 0u); }
@vertex fn vs_pellet_half(v: PelletIn, @builtin(vertex_index) i: u32) -> VertexOut { return pellet_vertex(v, i, 1u); }
@vertex fn vs_pellet_fine(v: PelletIn, @builtin(vertex_index) i: u32) -> VertexOut { return pellet_vertex(v, i, 2u); }
@vertex fn vs_pellet_coarse(v: PelletIn, @builtin(vertex_index) i: u32) -> VertexOut { return pellet_vertex(v, i, 3u); }
