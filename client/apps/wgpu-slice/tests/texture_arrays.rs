//! Authored GPU-only array/layer and queue-order regression; normal checks stay GPU-free.
#![cfg(not(target_arch = "wasm32"))]
use slither_client::presentation::{BodyAsset, BodyAssetStatus, BodyBlend, BodyRect};
use slither_wgpu_slice::{
    frame::{Draw, Frame},
    gpu::{Renderer, instance},
    materials::RgbaImage,
    scene::Resource,
};
use std::time::Duration;

fn resource(handle: u64, size: [u32; 2]) -> Resource {
    let [width, height] = size;
    let mut pixels = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let a = 64 + ((x + 3 * y) * 17 % 192) as u8;
            pixels.extend(if handle == 3 {
                [0, 0, 0, a]
            } else {
                [
                    a / (1 + handle % 3) as u8,
                    a / 2,
                    a / (1 + handle % 5) as u8,
                    a,
                ]
            });
        }
    }
    Resource {
        food_glow: false,
        food_outline: false,
        asset: BodyAsset {
            handle,
            revision: 1,
            width,
            height,
            status: BodyAssetStatus::Ready,
        },
        image: RgbaImage {
            width,
            height,
            pixels,
        }
        .into(),
    }
}
fn draw(asset: BodyAsset, i: usize) -> Draw {
    Draw {
        asset,
        source: Some(BodyRect {
            x: 0.25,
            y: 0.5,
            width: f64::from(asset.width) - 1.,
            height: f64::from(asset.height) - 1.,
        }),
        destination: BodyRect {
            x: -4.,
            y: -5.,
            width: 31.25,
            height: 25.75,
        },
        translation: [16. + (i % 3) as f64 * 4., 16. + (i % 5) as f64 * 3.],
        rotation: i as f64 * 0.125,
        effective_alpha: 0.7,
        blend: BodyBlend::SourceOver,
        repeat_uv: None,
        black_effect: true,
        corners: None,
    }
}
fn target(r: &Renderer) -> wgpu::Texture {
    r.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: 64,
            height: 64,
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
fn pixels(r: &Renderer, texture: &wgpu::Texture) -> Vec<u8> {
    let buffer = r.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 64 * 64 * 4,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut e = r.device.create_command_encoder(&Default::default());
    e.copy_texture_to_buffer(
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
                bytes_per_row: Some(256),
                rows_per_image: Some(64),
            },
        },
        texture.size(),
    );
    r.queue.submit([e.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer.slice(..).map_async(wgpu::MapMode::Read, move |v| {
        tx.send(v).unwrap();
    });
    r.device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(Duration::from_secs(10)),
        })
        .unwrap();
    rx.recv().unwrap().unwrap();
    buffer.slice(..).get_mapped_range().unwrap().to_vec()
}

#[test]
#[ignore = "explicit finite offscreen GPU check"]
fn ordered_arrays_match_individual_bindings_and_restore_retained_revisions() {
    let mut r = pollster::block_on(Renderer::new(
        &instance(),
        None,
        wgpu::TextureFormat::Rgba8Unorm,
    ))
    .unwrap();
    println!("array regression: {:?}", r.adapter_info);
    let scope = r.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut resources: Vec<_> = (1..=5)
        .map(|i| resource(i, if i == 5 { [5, 3] } else { [8, 8] }))
        .collect();
    let mut frame = Frame {
        identity: 1,
        tick: 0,
        extent: [64, 64],
        viewport_scale: 1.,
        snakes: vec![],
        draws: (0..80).map(|i| draw(resources[i % 4].asset, i)).collect(),
    };
    // Clamp, repeat, black source-over, additive, crop and rotation share the
    // same ordered geometry; a texture layer must never interpolate between quads.
    for (i, d) in frame.draws.iter_mut().enumerate() {
        if i >= 40 {
            d.blend = BodyBlend::Lighter;
        }
        if i >= 60 {
            d.source = None;
            d.repeat_uv = Some(BodyRect {
                x: -1.25,
                y: 0.75,
                width: 2.5,
                height: 3.,
            });
        }
    }
    frame.draws.push(draw(resources[4].asset, 0));
    let a = target(&r);
    let b = target(&r);
    let c = target(&r);
    let d = target(&r);
    for resource in &resources {
        r.upload(resource).unwrap();
    }
    r.render(&frame, &a.create_view(&Default::default()))
        .unwrap();
    let individual = r.draw_call_count();
    r.sync_resources(&resources).unwrap();
    r.render(&frame, &b.create_view(&Default::default()))
        .unwrap();
    assert!(r.draw_call_count() < individual / 2);
    let old = resources.clone();
    resources[0] = resource(1, [8, 8]);
    resources[0].asset.revision = 2;
    std::sync::Arc::make_mut(&mut resources[0].image)
        .pixels
        .fill(255);
    r.sync_resources(&resources).unwrap();
    frame
        .draws
        .iter_mut()
        .filter(|d| d.asset.handle == 1)
        .for_each(|d| d.asset = resources[0].asset);
    r.render(&frame, &c.create_view(&Default::default()))
        .unwrap();
    // Resync an older immutable picture before waiting for any preceding work.
    r.sync_resources(&old).unwrap();
    frame
        .draws
        .iter_mut()
        .filter(|d| d.asset.handle == 1)
        .for_each(|d| d.asset = old[0].asset);
    r.render(&frame, &d.create_view(&Default::default()))
        .unwrap();
    let expected = pixels(&r, &a);
    assert_eq!(expected, pixels(&r, &b));
    assert_eq!(expected, pixels(&r, &d));
    assert_ne!(expected, pixels(&r, &c));
    let count = r.device.limits().max_texture_array_layers + 1;
    let many: Vec<_> = (0..count)
        .map(|i| resource(100 + u64::from(i), [2, 2]))
        .collect();
    r.sync_resources(&many).unwrap();
    let wide = Frame {
        draws: many.iter().map(|r| draw(r.asset, 0)).collect(),
        ..frame.clone()
    };
    r.render(&wide, &b.create_view(&Default::default()))
        .unwrap();
    assert_eq!(r.draw_call_count(), 2);
    r.sync_resources(&old).unwrap();
    let mut bad = old.clone();
    bad[0].asset.width = 9;
    assert!(r.sync_resources(&bad).is_err());
    r.render(&frame, &b.create_view(&Default::default()))
        .unwrap();
    assert_eq!(expected, pixels(&r, &b));
    r.sync_resources(&[]).unwrap();
    assert!(
        r.render(&frame, &b.create_view(&Default::default()))
            .is_err()
    );
    assert!(pollster::block_on(scope.pop()).is_none());
}
