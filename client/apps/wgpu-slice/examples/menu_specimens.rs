//! Finite native offscreen controls, without windows, servers or external assets.
use slither_wgpu_slice::{
    Result,
    cosmetics::{Cosmetics, Images, MenuArt},
    gpu::{Renderer, instance},
    menu::{Menu, MenuFrame},
    menu_specimen,
    text::TextCache,
};
use std::{io::Cursor, path::Path, sync::mpsc, time::Duration};

fn capture(renderer: &mut Renderer, p: &MenuFrame, file: &Path) -> Result<()> {
    renderer.sync_resources(&p.resources)?;
    let [width, height] = p.frame.extent;
    let texture = renderer.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("menu specimen"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    renderer.render(&p.frame, &texture.create_view(&Default::default()))?;
    let row = (width * 4).div_ceil(256) * 256;
    let buffer = renderer.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("menu readback"),
        size: u64::from(row) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = renderer.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
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
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    renderer.queue.submit([encoder.finish()]);
    let (tx, rx) = mpsc::channel();
    buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    renderer
        .device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(Duration::from_secs(20)),
        })
        .map_err(|e| e.to_string())?;
    rx.recv_timeout(Duration::from_secs(1))
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let data = buffer
        .slice(..)
        .get_mapped_range()
        .map_err(|e| e.to_string())?;
    let pixels: Vec<_> = data
        .chunks_exact(row as usize)
        .flat_map(|r| r[..width as usize * 4].iter().copied())
        .collect();
    image::save_buffer(file, &pixels, width, height, image::ColorType::Rgba8)
        .map_err(|e| e.to_string())?;
    drop(data);
    buffer.unmap();
    println!(
        "{} draws={} pixel_bytes={}",
        file.display(),
        p.frame.draws.len(),
        p.resources
            .iter()
            .map(|r| r.image.pixels.len())
            .sum::<usize>()
    );
    Ok(())
}
fn synthetic_logo(color: [u8; 4]) -> Result<Vec<u8>> {
    let mut bytes = Cursor::new(Vec::new());
    // Deliberately synthetic icon; no external reference/brand artwork.
    let image = image::RgbaImage::from_fn(320, 80, |x, y| {
        let inside = ((x as f64 - 160.) / 155.).powi(2) + ((y as f64 - 40.) / 35.).powi(2) < 1.;
        image::Rgba(if inside { color } else { [0; 4] })
    });
    image
        .write_to(&mut bytes, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(bytes.into_inner())
}
async fn run(output: &Path) -> Result<()> {
    std::fs::create_dir(output).map_err(|e| e.to_string())?;
    let mut renderer = Renderer::new(&instance(), None, wgpu::TextureFormat::Rgba8Unorm).await?;
    println!(
        "adapter={:?} samples={}",
        renderer.adapter_info, renderer.sample_count
    );
    let validation = renderer
        .device
        .push_error_scope(wgpu::ErrorFilter::Validation);
    let mut text = TextCache::default();
    let mut menu = Menu::default();
    for dpr in [1., 1.25, 2., 3.] {
        for (state, name) in ["normal", "hover", "pressed", "disabled", "spinner"]
            .iter()
            .enumerate()
        {
            let extent = [(1280. * dpr) as u32, (720. * dpr) as u32];
            let view = menu_specimen::view(state as u8)?;
            let layout = menu_specimen::layout(&mut text, &view, extent, dpr)?;
            let p = menu.prepare(&mut text, &view, &layout, extent, dpr, None, 0)?;
            capture(
                &mut renderer,
                &p,
                &output.join(format!("native-{name}-dpr-{dpr}.png")),
            )?;
        }
    }
    let view = menu_specimen::view(0)?;
    let mut images = Images::default();
    images.add("menu/logo.png", &synthetic_logo([90, 180, 120, 255])?)?;
    let mut art = Cosmetics::from_images(images, None)?;
    let layout = menu_specimen::layout(&mut text, &view, [1280, 720], 1.)?;
    let old = menu.prepare(&mut text, &view, &layout, [1280, 720], 1., Some(&art), 0)?;
    capture(
        &mut renderer,
        &old,
        &output.join("native-synthetic-art.png"),
    )?;
    art.replace_menu_art(MenuArt::Logo(0), &synthetic_logo([170, 90, 180, 255])?)?;
    let replacement = menu.prepare(&mut text, &view, &layout, [1280, 720], 1., Some(&art), 0)?;
    capture(
        &mut renderer,
        &replacement,
        &output.join("native-replacement.png"),
    )?;
    let layout = menu_specimen::layout(&mut text, &view, [960, 540], 1.)?;
    let resized = menu.prepare(&mut text, &view, &layout, [960, 540], 1., None, 0)?;
    capture(
        &mut renderer,
        &resized,
        &output.join("native-resize-missing-art.png"),
    )?;
    // A fresh renderer exercises an old immutable prepared picture after retirement.
    let mut retained_renderer =
        Renderer::new(&instance(), None, wgpu::TextureFormat::Rgba8Unorm).await?;
    capture(
        &mut retained_renderer,
        &old,
        &output.join("native-retained-after-resize.png"),
    )?;
    let layout = menu_specimen::layout(&mut text, &view, [640, 360], 1.)?;
    capture(
        &mut renderer,
        &menu.prepare(&mut text, &view, &layout, [640, 360], 1., None, 0)?,
        &output.join("native-short-640x360.png"),
    )?;
    if let Some(error) = validation.pop().await {
        return Err(error.to_string());
    }
    Ok(())
}
fn main() {
    let result = std::env::args_os()
        .nth(1)
        .ok_or_else(|| "new output directory required".to_owned())
        .and_then(|p| pollster::block_on(run(Path::new(&p))));
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
