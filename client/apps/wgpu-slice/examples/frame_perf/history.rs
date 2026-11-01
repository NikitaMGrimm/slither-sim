//! Common authored geometry conditions the diagnostic VBO pool, not production policy.
use super::*;
use slither_client::presentation::{BodyAsset, BodyAssetStatus, BodyBlend, BodyRect};
use slither_wgpu_slice::{
    frame::{Draw, Frame, VERTEX_CHUNK_QUADS},
    materials::RgbaImage,
    scene::Resource,
};

// The bounded 120/500-model fixture fits nine chunks. Assert this ceiling in
// the driver rather than letting either side silently establish a larger pool.
pub const QUADS: usize = 9 * VERTEX_CHUNK_QUADS;

pub fn warm(r: &mut gpu::Renderer) -> Result<()> {
    let asset = BodyAsset {
        handle: u64::MAX,
        revision: 1,
        width: 1,
        height: 1,
        status: BodyAssetStatus::Ready,
    };
    let resource = Resource {
        asset,
        image: std::sync::Arc::new(RgbaImage {
            width: 1,
            height: 1,
            pixels: vec![255; 4],
        }),
    };
    r.sync_resources(&[resource])?;
    let texture = r.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("shared-history one-pixel target"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let draw = Draw {
        corners: None,
        asset,
        source: None,
        repeat_uv: None,
        black_effect: false,
        destination: BodyRect {
            x: 0.,
            y: 0.,
            width: 1.,
            height: 1.,
        },
        translation: [0., 0.],
        rotation: 0.,
        effective_alpha: 1.,
        blend: BodyBlend::SourceOver,
    };
    let frame = Frame {
        identity: 0,
        extent: [1, 1],
        viewport_scale: 1.,
        tick: 0,
        draws: vec![draw; QUADS],
        snakes: Vec::new(),
    };
    r.begin_diagnostic_frame();
    r.render_world_at(
        &frame,
        &texture.create_view(&Default::default()),
        [1, 1],
        r.sample_count,
    )?;
    // The shared upload fills every chunk, including inactive chunks in later
    // small views. Wait outside timing before dropping its sole placeholder.
    wait(r)?;
    r.sync_resources(&[])?;
    Ok(())
}
