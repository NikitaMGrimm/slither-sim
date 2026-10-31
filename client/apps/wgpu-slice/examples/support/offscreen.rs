//! Finite authored fixtures and readback, shared by evidence and GPU regressions.
use slither_client::presentation::{BodyAsset, BodyAssetStatus, BodyBlend, BodyRect};
use slither_wgpu_slice::{
    Result,
    frame::{Draw, Frame},
    gpu::Renderer,
    materials::RgbaImage,
    scene::Resource,
};

pub fn target(renderer: &Renderer, extent: [u32; 2]) -> wgpu::Texture {
    renderer.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("authored offscreen presentation image"),
        size: wgpu::Extent3d {
            width: extent[0],
            height: extent[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}
pub fn pixels(renderer: &Renderer, texture: &wgpu::Texture) -> Result<Vec<u8>> {
    let [width, height] = [texture.width(), texture.height()];
    let row = (width * 4).div_ceil(256) * 256;
    let buffer = renderer.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("finite evidence readback"),
        size: u64::from(row) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = renderer.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(height),
            },
        },
        texture.size(),
    );
    renderer.queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    renderer
        .device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(30)),
        })
        .map_err(|e| e.to_string())?;
    rx.recv_timeout(std::time::Duration::from_secs(1))
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let mapped = buffer
        .slice(..)
        .get_mapped_range()
        .map_err(|e| e.to_string())?;
    let pixels = mapped
        .chunks_exact(row as usize)
        .flat_map(|r| r[..width as usize * 4].iter().copied())
        .collect();
    drop(mapped);
    buffer.unmap();
    Ok(pixels)
}
pub fn scene(physical: [u32; 2]) -> (Frame, Frame, Vec<Resource>) {
    let resources: Vec<_> = [
        [230, 220, 185, 255],
        [78, 180, 153, 255],
        [185, 105, 210, 255],
    ]
    .into_iter()
    .enumerate()
    .map(|(i, color)| Resource {
        asset: BodyAsset {
            handle: i as u64 + 1,
            revision: 1,
            width: 1,
            height: 1,
            status: BodyAssetStatus::Ready,
        },
        image: RgbaImage {
            width: 1,
            height: 1,
            pixels: color.to_vec(),
        }
        .into(),
    })
    .collect();
    let draw = |asset: BodyAsset, destination: BodyRect, rotation: f64| Draw {
        asset,
        source: None,
        repeat_uv: None,
        black_effect: false,
        destination: BodyRect {
            x: 0.,
            y: 0.,
            ..destination
        },
        translation: [destination.x, destination.y],
        rotation,
        effective_alpha: 1.,
        blend: BodyBlend::SourceOver,
        corners: None,
    };
    let mut world = Frame {
        identity: 1,
        tick: 1,
        extent: [1500, 845],
        viewport_scale: 1.,
        snakes: Vec::new(),
        draws: Vec::new(),
    };
    // Thin diagonals and polygonal curves expose raster coverage, with no art input.
    for i in 0..48 {
        let t = f64::from(i) / 47.;
        world.draws.push(draw(
            resources[i as usize % 3].asset,
            BodyRect {
                x: 750.,
                y: 422.5,
                width: 640. + 100. * t,
                height: 0.65 + 2. * t,
            },
            t * std::f64::consts::TAU,
        ));
    }
    for i in 0..96 {
        let a = f64::from(i) * std::f64::consts::TAU / 96.;
        world.draws.push(draw(
            resources[1].asset,
            BodyRect {
                x: 750. + 240. * a.cos(),
                y: 422.5 + 240. * a.sin(),
                width: 17.,
                height: 2.,
            },
            a + std::f64::consts::FRAC_PI_2,
        ));
    }
    let overlay = Frame {
        identity: 2,
        tick: 1,
        extent: physical,
        viewport_scale: 1.,
        snakes: Vec::new(),
        draws: vec![draw(
            resources[1].asset,
            BodyRect {
                x: 16.,
                y: 16.,
                width: 96.,
                height: 8.,
            },
            0.,
        )],
    };
    (world, overlay, resources)
}
