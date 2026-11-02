//! Finite native offscreen controls, without windows, servers or external assets.
use slither_wgpu_slice::Result;
#[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
use slither_wgpu_slice::{
    cosmetics::{Cosmetics, Images},
    frame::Frame,
    gpu::{Renderer, instance},
    menu::MenuFrame,
    scene::Resource,
};
use std::path::Path;
#[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
use std::{io::Cursor, sync::mpsc, time::Duration};

/// The page and world cover below egui, as a frame the renderer can draw.
#[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
fn page_frame(picture: &MenuFrame) -> Frame {
    Frame {
        identity: 0,
        tick: 0,
        extent: picture.extent,
        viewport_scale: picture.viewport_scale,
        draws: picture
            .backdrop
            .iter()
            .chain(&picture.world_cover)
            .cloned()
            .collect(),
        snakes: Vec::new(),
        food_batch: None,
    }
}
#[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
fn readback_with(
    renderer: &mut Renderer,
    frame: &Frame,
    resources: &[Resource],
    paint: impl FnOnce(&mut Renderer, &wgpu::TextureView),
) -> Result<Vec<u8>> {
    renderer.sync_resources(resources)?;
    let [width, height] = frame.extent;
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
    renderer.render_overlay(frame, &texture.create_view(&Default::default()))?;
    paint(renderer, &texture.create_view(&Default::default()));
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
    let mut pixels = vec![0; width as usize * height as usize * 4];
    for (destination, source) in pixels
        .chunks_exact_mut(width as usize * 4)
        .zip(data.chunks_exact(row as usize))
    {
        destination.copy_from_slice(&source[..width as usize * 4]);
    }
    drop(data);
    buffer.unmap();
    Ok(pixels)
}
/// PNG stores straight alpha; production textures retain premultiplied pixels.
fn straight_alpha(premultiplied: &[u8]) -> Vec<u8> {
    let mut pixels = premultiplied.to_vec();
    for pixel in pixels.as_chunks_mut::<4>().0 {
        let alpha = u32::from(pixel[3]);
        for channel in &mut pixel[..3] {
            *channel = (u32::from(*channel) * 255 + alpha / 2)
                .checked_div(alpha)
                .unwrap_or(0)
                .min(255) as u8;
        }
    }
    pixels
}
#[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
fn synthetic_icon(color: [u8; 4]) -> Result<Vec<u8>> {
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
    #[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
    let mut renderer = Renderer::new(&instance(), None, wgpu::TextureFormat::Rgba8Unorm).await?;
    #[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
    if std::env::args().nth(2).as_deref() == Some("--focus") {
        return focus::specimens(&mut renderer, output).await;
    }
    #[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
    if std::env::args().nth(2).as_deref() == Some("--login") {
        if std::env::args_os().nth(3).is_some() {
            return Err("--login no longer accepts an art directory".into());
        }
        return login_specimens(output, &mut renderer);
    }
    #[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
    if std::env::args().nth(2).as_deref() == Some("--settings") {
        return settings_trial(output, &mut renderer);
    }
    #[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
    if std::env::args().nth(2).as_deref() == Some("--builder") {
        return builder::specimens(&mut renderer, output);
    }
    #[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
    if std::env::args().nth(2).as_deref() == Some("--server-picker") {
        return server_picker::specimens(&mut renderer, output).await;
    }
    #[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
    if std::env::args().nth(2).as_deref() == Some("--studio") {
        return studio_specimens(&mut renderer, output);
    }
    // The native login host is not built here; never substitute default specimens.
    #[cfg(any(target_arch = "wasm32", target_os = "linux"))]
    if std::env::args().nth(2).as_deref() == Some("--studio") {
        return Err("--studio needs the native login host (not built on Linux or wasm)".into());
    }
    #[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
    if std::env::args().nth(2).as_deref() == Some("--champion") {
        return champion::specimens(&mut renderer, output);
    }
    if std::env::args().nth(2).as_deref() == Some("--stroke") {
        return stroke_specimens(output);
    }
    Err("Mode required: --login, --settings, --server-picker, --studio, --builder, --focus, --champion or --stroke".into())
}

#[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
fn settings_trial(output: &Path, renderer: &mut Renderer) -> Result<()> {
    use slither_wgpu_slice::login_native::NativeLogin;
    use std::time::Instant;
    let mut rows = Vec::new();
    for extent in [[960, 540], [360, 640]] {
        let mut login = NativeLogin::new("Player")?;
        for now in (0..=9600).step_by(16) {
            login.step(now as f64, true, false)?;
        }
        login.open_settings(true);
        for now in (9616..=10000).step_by(16) {
            login.step(now as f64, true, false)?;
            login.prepare_host(extent, 1., true, None)?;
        }
        let mut times = Vec::new();
        for _ in 0..1000 {
            let start = Instant::now();
            std::hint::black_box(login.prepare_host(extent, 1., true, None)?);
            times.push(start.elapsed().as_nanos() as u64);
        }
        times.sort_unstable();
        let picture = login.prepare_host(extent, 1., true, None)?;
        capture_settings(
            renderer,
            &picture,
            &mut login,
            &output.join(format!("settings-{}x{}.png", extent[0], extent[1])),
        )?;
        let texture = renderer.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("settings trial"),
            size: wgpu::Extent3d {
                width: extent[0],
                height: extent[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let target = texture.create_view(&Default::default());
        let mut frame_times = Vec::new();
        for _ in 0..120 {
            let start = Instant::now();
            let picture = login.prepare_host(extent, 1., true, None)?;
            renderer.sync_resources(&picture.resources)?;
            renderer.render_overlay(&page_frame(&picture), &target)?;
            login.egui.paint(renderer, &target);
            frame_times.push(start.elapsed().as_nanos() as u64);
            renderer
                .device
                .poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: Some(Duration::from_secs(10)),
                })
                .map_err(|e| e.to_string())?;
        }
        frame_times.sort_unstable();
        rows.push(serde_json::json!({"extent":extent,"samples":times.len(),"prepare_median_ns":times[500],"prepare_p95_ns":times[950],"frame_samples":frame_times.len(),"frame_cpu_median_ns":frame_times[60],"frame_cpu_p95_ns":frame_times[114],"adapter":format!("{:?}",renderer.adapter_info.device_type)}));
    }
    std::fs::write(
        output.join("timings.json"),
        serde_json::to_vec_pretty(&rows).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
#[cfg(not(target_os = "linux"))]
/// The studio as the desktop host presents it: page, egui login and studio,
/// then the preview in the top layer.
#[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
fn studio_picture(
    login: &mut slither_wgpu_slice::login_native::NativeLogin,
    extent: [u32; 2],
    dpr: f64,
    admission: bool,
    art: Option<&mut Cosmetics>,
) -> Result<(MenuFrame, Frame, [usize; 3])> {
    let logical = extent.map(|n| f64::from(n) / dpr);
    let art = match art {
        Some(art) => {
            login.studio.sync_art(logical, Some(&mut *art))?;
            Some(&*art)
        }
        None => None,
    };
    let mut picture = login.prepare_host(extent, dpr, admission, art)?;
    let mut top = Frame {
        draws: Vec::new(),
        ..page_frame(&picture)
    };
    let mut preview = [0; 3];
    if login.studio.state.visible() {
        let (frame, resources) = login.studio.prepare_preview(extent, dpr, art)?;
        preview = [
            frame.draws.len(),
            resources.len(),
            resources.iter().map(|r| r.image.pixels.len()).sum(),
        ];
        top = frame;
        picture.resources.extend(resources);
    }
    Ok((picture, top, preview))
}

#[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
fn studio_specimens(renderer: &mut Renderer, output: &Path) -> Result<()> {
    use slither_client::{
        presentation::{FaceGeometry, resolve_skin},
        skin_studio::*,
    };
    use slither_wgpu_slice::{login_native::NativeLogin, menu_storage::MenuStorage};
    let png = synthetic_icon([180, 100, 210, 255])?;
    let mut full = Cosmetics::load(
        |path| {
            if slither_wgpu_slice::cosmetics::BackgroundArt::ALL
                .iter()
                .any(|art| art.path() == path)
                || path.ends_with(".json")
            {
                Err("skin-only authored fixture".into())
            } else {
                Ok(png.clone())
            }
        },
        None,
    )?;
    let mut ledger = vec![
        "dpr,skin,art,clock_ms,egui_shapes,egui_textures,preview_draws,preview_slots,preview_bytes"
            .to_owned(),
    ];
    for scale in [1., 1.5, 2., 3., 4., 5.] {
        let extent = [(960. * scale) as u32, (540. * scale) as u32];
        for skin in 0..=65 {
            for art in ["fallback", "synthetic-ready"] {
                // Each fixture owns a fresh login and handle allocators.
                renderer.sync_resources(&[])?;
                let mut login = NativeLogin::with_preferences(
                    AppearanceRecord {
                        builtin: skin,
                        ..Default::default()
                    },
                    MenuStorage::at(None),
                )?;
                login.step(0., false, false)?;
                login.open_studio();
                for time in [0., 230., 355., 730.] {
                    login.step(time, false, false)?;
                    let sources = (art != "fallback").then_some(&mut full);
                    let (picture, top, [preview, slots, bytes]) =
                        studio_picture(&mut login, extent, scale, false, sources)?;
                    let textures = login
                        .egui
                        .context()
                        .map_or(0, |c| c.tex_manager().read().num_allocated());
                    ledger.push(format!(
                        "{scale},{skin},{art},{time},{},{textures},{preview},{slots},{bytes}",
                        login.egui.prepared_shapes().len(),
                    ));
                    if preview > 1024 {
                        return Err("Studio preview allowance exceeded".into());
                    }
                    let requested = resolve_skin(skin, &[], false).map_err(|e| e.to_string())?;
                    let special =
                        requested.antenna.is_some() || requested.face != FaceGeometry::default();
                    if scale <= 2. && time == 730. && (skin == 0 || skin == 65 || special) {
                        capture_layered(
                            renderer,
                            &picture,
                            &top,
                            &mut login,
                            &output.join(format!("studio-{skin:02}-{art}-{scale}x.png")),
                        )?;
                    }
                    login.studio.presented();
                }
            }
        }
    }
    std::fs::write(output.join("dpr-ledger.csv"), ledger.join("\n") + "\n")
        .map_err(|e| e.to_string())?;
    renderer.sync_resources(&[])?;
    let mut login = NativeLogin::new("Player")?;
    for now in (0..=9600).step_by(16) {
        login.step(f64::from(now), false, false)?;
    }
    login.prepare_host([960, 540], 1., false, None)?;
    login.open_studio();
    let mut strip = image::RgbaImage::new(960 * 4, 540 * 2);
    for (index, time) in [0., 58., 115., 230., 288., 345., 459., 460.]
        .into_iter()
        .enumerate()
    {
        if index == 4 {
            login.studio.state.cancel(9830.);
        }
        login.step(9600. + time, false, false)?;
        let (picture, top, _) = studio_picture(&mut login, [960, 540], 1., false, None)?;
        let path = output.join(format!("transition-{time:03}ms.png"));
        capture_layered(renderer, &picture, &top, &mut login, &path)?;
        login.studio.presented();
        let frame = image::open(&path).map_err(|e| e.to_string())?.to_rgba8();
        image::imageops::replace(
            &mut strip,
            &frame,
            (index % 4 * 960) as i64,
            (index / 4 * 540) as i64,
        );
    }
    strip
        .save(output.join("entry-return-strip.png"))
        .map_err(|e| e.to_string())?;
    renderer.sync_resources(&[])?;
    let mut narrow = NativeLogin::new("")?;
    narrow.step(0., false, false)?;
    narrow.open_studio();
    narrow.step(230., false, false)?;
    let (picture, top, _) = studio_picture(&mut narrow, [360, 640], 1., false, None)?;
    capture_layered(
        renderer,
        &picture,
        &top,
        &mut narrow,
        &output.join("studio-narrow-360x640.png"),
    )?;
    for (name, logical) in [("narrow", [360., 640.]), ("short-wide", [960., 360.])] {
        for scale in [1., 1.5, 2.] {
            renderer.sync_resources(&[])?;
            let mut login = NativeLogin::new("")?;
            login.step(0., false, false)?;
            login.open_studio();
            login.step(230., false, false)?;
            login.studio.status = "Saved for this run".into();
            let extent = logical.map(|n| (n * scale) as u32);
            let (picture, top, _) = studio_picture(&mut login, extent, scale, false, None)?;
            capture_layered(
                renderer,
                &picture,
                &top,
                &mut login,
                &output.join(format!("studio-{name}-status-{scale}x.png")),
            )?;
        }
    }
    // Entry over the settled login, as the login's egui pass composes it.
    for (extent, dpr) in [([960, 720], 1.), ([1440, 1080], 1.5), ([1920, 1440], 2.)] {
        renderer.sync_resources(&[])?;
        let mut login = NativeLogin::new("Player")?;
        for now in (0..=9600).step_by(16) {
            login.step(now as f64, true, false)?;
        }
        login.prepare_host(extent, dpr, true, None)?;
        let (picture, top, _) = studio_picture(&mut login, extent, dpr, true, None)?;
        capture_layered(
            renderer,
            &picture,
            &top,
            &mut login,
            &output.join(format!("login-{dpr}x.png")),
        )?;
        login.open_studio();
        let mut now = 9616;
        for (name, alpha) in [("entering", 0.5), ("open", 1.)] {
            while login.studio.state.alpha() < alpha {
                login.step(f64::from(now), true, false)?;
                now += 16;
            }
            let (picture, top, _) = studio_picture(&mut login, extent, dpr, true, None)?;
            capture_layered(
                renderer,
                &picture,
                &top,
                &mut login,
                &output.join(format!("studio-{name}-{dpr}x.png")),
            )?;
            login.studio.presented();
        }
    }
    std::fs::write(output.join("README.txt"),"Authored offline specimens; no windows, sockets or official art. Synthetic-ready bulbs and decorations are placeholder images.\nThe studio is egui, composed as the desktop host presents it: page, egui login and studio, then the preview above them.\nLogical 960x540 at DPR 1, 1.5, 2. Numeric ledger additionally covers DPR 3, 4, 5.\nStrip: row 1 entry at 0,58,115,230 ms; row 2 return at 288,345,459,460 ms; Cancel at 230 ms.\nlogin/studio-entering/studio-open: 960x720 logical over the settled login at DPR 1, 1.5, 2.\nOwner appearance, physical input, display pacing and server acceptance are unverified.\n").map_err(|e|e.to_string())?;
    Ok(())
}
/// Authored string/raster probes at frozen core times.
/// Output stays external; optional private reference composition is a separate
/// local operation and never part of this example's inputs or repository data.
fn stroke_specimens(output: &Path) -> Result<()> {
    use slither_client::{
        menu_layout::MenuRect,
        menu_preferences::MenuQuality,
        start_page::{MenuLocale, StartMenuState},
    };
    use slither_wgpu_slice::wordmark;
    let holder = MenuRect {
        x: 735.,
        y: 50.,
        width: 450.,
        height: 115.,
    };
    let mut timings = Vec::new();
    for dpr in [1., 2., 5.] {
        let layout = wordmark::layout(wordmark::DEFAULT_TEXT, holder, 1920.)?;
        for progress in [0.15, 0.5, 1.] {
            let mut runs = Vec::new();
            for index in 0..7 {
                let now = std::time::Instant::now();
                let raster = wordmark::raster(&layout, dpr, progress)?;
                let ms = now.elapsed().as_secs_f64() * 1000.;
                let nonzero = raster
                    .pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .filter(|p| p[3] > 0)
                    .count();
                if nonzero == 0 {
                    return Err("empty timed raster".into());
                }
                if index > 0 {
                    runs.push(ms);
                }
            }
            timings.push(serde_json::json!({"dpr": dpr, "progress": progress,
                "milliseconds": runs, "size": wordmark::raster_size(&layout, dpr)?,
                "profile": if cfg!(debug_assertions) { "dev" } else { "release" }}));
        }
    }
    for (index, value) in [
        wordmark::DEFAULT_TEXT,
        "Slither",
        "Hello World 123",
        "AnExtremelyLongSnakeClientName",
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ",
        "abcdefghijklmnopqrstuvwxyz",
        "0123456789 !?.,:;+-_()[]{}@#$%&*=<>/\\|~^`\"'",
    ]
    .into_iter()
    .enumerate()
    {
        let layout = wordmark::layout(value, holder, 1920.)?;
        let raster = wordmark::raster(&layout, 1., 1.)?;
        image::save_buffer(
            output.join(format!("string-{index}.png")),
            &straight_alpha(&raster.pixels),
            raster.width,
            raster.height,
            image::ColorType::Rgba8,
        )
        .map_err(|e| e.to_string())?;
    }
    // A ten-fps CPU raster sequence preserves the shared clock's timing for an
    // external motion strip/video.
    let motion = output.join("motion");
    std::fs::create_dir(&motion).map_err(|e| e.to_string())?;
    let geometry = wordmark::layout(wordmark::DEFAULT_TEXT, holder, 1920.)?;
    let mut state = StartMenuState::new(MenuLocale::English, true, MenuQuality::High);
    for frame in 0..=72 {
        let ms = frame * 100;
        if ms > 0 {
            for now in ((ms - 100 + 16)..ms).step_by(16) {
                state.step(f64::from(now), false).map_err(str::to_owned)?;
            }
        }
        state.step(f64::from(ms), false).map_err(str::to_owned)?;
        let view = state.view(true);
        let progress = view.logo_reveal;
        let mut raster = wordmark::raster(&geometry, 1., progress)?;
        let opacity = view.login_alpha * view.logo_alpha;
        for p in raster.pixels.as_chunks_mut::<4>().0 {
            let alpha = f64::from(p[3]) / 255. * opacity;
            for (channel, background) in p[..3].iter_mut().zip([22., 28., 34.]) {
                *channel =
                    (f64::from(*channel) * opacity + background * (1. - alpha)).round() as u8;
            }
            p[3] = 255;
        }
        image::save_buffer(
            motion.join(format!("{frame:03}.png")),
            &raster.pixels,
            raster.width,
            raster.height,
            image::ColorType::Rgba8,
        )
        .map_err(|e| e.to_string())?;
    }
    std::fs::write(
        output.join("raster-timings.json"),
        serde_json::to_vec_pretty(&timings).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn png_conversion_preserves_the_production_raster() {
        let pixels = [40, 20, 10, 128, 5, 6, 7, 255, 0, 0, 0, 0];
        let copy = pixels;
        assert_eq!(
            straight_alpha(&pixels),
            [80, 40, 20, 128, 5, 6, 7, 255, 0, 0, 0, 0]
        );
        assert_eq!(pixels, copy);
    }
}
#[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
#[path = "menu_specimens/builder.rs"]
mod builder;

#[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
#[path = "menu_specimens/focus.rs"]
mod focus;

#[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
#[path = "menu_specimens/server_picker.rs"]
mod server_picker;

#[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
#[path = "menu_specimens/champion.rs"]
mod champion;

/// The desktop host's layer order without a game world: page, world cover,
/// egui, then the top layer (the studio preview) above it.
#[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
fn capture_layered(
    renderer: &mut Renderer,
    picture: &MenuFrame,
    top: &Frame,
    login: &mut slither_wgpu_slice::login_native::NativeLogin,
    path: &Path,
) -> Result<()> {
    let frame = page_frame(picture);
    let mut layer = Ok(());
    let pixels = readback_with(renderer, &frame, &picture.resources, |r, v| {
        login.egui.paint(r, v);
        layer = r.render_overlay(top, v);
    })?;
    layer?;
    image::save_buffer(
        path,
        &pixels,
        frame.extent[0],
        frame.extent[1],
        image::ColorType::Rgba8,
    )
    .map_err(|e| e.to_string())
}
/// A screen with nothing in the top layer.
#[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
fn capture_settings(
    renderer: &mut Renderer,
    picture: &MenuFrame,
    login: &mut slither_wgpu_slice::login_native::NativeLogin,
    path: &Path,
) -> Result<()> {
    let top = Frame {
        draws: Vec::new(),
        ..page_frame(picture)
    };
    capture_layered(renderer, picture, &top, login, path)
}

#[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
fn login_specimens(output: &Path, renderer: &mut Renderer) -> Result<()> {
    use slither_wgpu_slice::login_native::NativeLogin;
    for (extent, dpr) in [([960, 540], 1.), ([360, 640], 1.), ([720, 1280], 2.)] {
        let mut login = NativeLogin::new("Player")?;
        for now in (0..=9600).step_by(16) {
            login.step(now as f64, true, false)?;
        }
        login.prepare_host(extent, dpr, true, None)?;
        let picture = login.prepare_host(extent, dpr, true, None)?;
        capture_settings(
            renderer,
            &picture,
            &mut login,
            &output.join(format!("login-{}x{}.png", extent[0], extent[1])),
        )?;
    }
    Ok(())
}
