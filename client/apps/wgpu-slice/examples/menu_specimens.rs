//! Finite native offscreen controls, without windows, servers or external assets.
use slither_wgpu_slice::{
    Result,
    cosmetics::{Cosmetics, Images, MenuArt},
    frame::Frame,
    gpu::{Renderer, instance},
    menu::{Menu, MenuFrame},
    menu_specimen,
    scene::Resource,
    text::TextCache,
};
use std::{io::Cursor, path::Path, sync::mpsc, time::Duration};

fn capture(renderer: &mut Renderer, p: &MenuFrame, file: &Path) -> Result<()> {
    capture_scaled(renderer, p, file, None)
}
fn capture_scaled(
    renderer: &mut Renderer,
    p: &MenuFrame,
    file: &Path,
    physical: Option<[u32; 2]>,
) -> Result<()> {
    let mut frame = p.frame.clone();
    if let Some(backdrop) = &p.backdrop {
        frame.draws.insert(0, backdrop.clone());
    }
    let pixels = readback(renderer, &frame, &p.resources, physical)?;
    let [width, height] = physical.unwrap_or(p.frame.extent);
    image::save_buffer(file, &pixels, width, height, image::ColorType::Rgba8)
        .map_err(|e| e.to_string())?;
    println!(
        "{} draws={} pixel_bytes={}",
        file.display(),
        frame.draws.len(),
        p.resources
            .iter()
            .map(|r| r.image.pixels.len())
            .sum::<usize>()
    );
    Ok(())
}
fn readback(
    renderer: &mut Renderer,
    frame: &Frame,
    resources: &[Resource],
    physical: Option<[u32; 2]>,
) -> Result<Vec<u8>> {
    readback_with(renderer, frame, resources, physical, |_, _| {})
}
fn readback_with(
    renderer: &mut Renderer,
    frame: &Frame,
    resources: &[Resource],
    physical: Option<[u32; 2]>,
    paint: impl FnOnce(&mut Renderer, &wgpu::TextureView),
) -> Result<Vec<u8>> {
    renderer.sync_resources(resources)?;
    let [width, height] = physical.unwrap_or(frame.extent);
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
    if physical.is_some() {
        let [w, h] = frame.extent;
        let backing = renderer.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("native host backing"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let source = backing.create_view(&Default::default());
        renderer.render(frame, &source)?;
        // Same linear final scaling as SurfaceHost::present_scaled.
        let blitter = wgpu::util::TextureBlitterBuilder::new(
            &renderer.device,
            wgpu::TextureFormat::Rgba8Unorm,
        )
        .sample_type(wgpu::FilterMode::Linear)
        .build();
        let mut encoder = renderer.device.create_command_encoder(&Default::default());
        blitter.copy(
            &renderer.device,
            &mut encoder,
            &source,
            &texture.create_view(&Default::default()),
        );
        renderer.queue.submit([encoder.finish()]);
    } else {
        renderer.render_overlay(frame, &texture.create_view(&Default::default()))?;
    }
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
    if std::env::args().nth(2).as_deref() == Some("--settings-trial") {
        return settings_trial(output, &mut renderer);
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
    if std::env::args().nth(2).as_deref() == Some("--champion") {
        use slither_client::{
            champion::ChampionState,
            menu_controls::PanelInputState,
            menu_input::{EditIntent, MenuInputState},
            menu_theme::MenuTheme,
            session_policy::{Command, SessionPolicy},
        };
        let mut session = SessionPolicy::new();
        session
            .prepare_controls(
                &[
                    Command {
                        opcode: b'a',
                        argument: 0,
                    },
                    Command {
                        opcode: b'v',
                        argument: 1,
                    },
                ],
                0.,
            )
            .map_err(|e| e.0)?;
        session.commit_controls().map_err(|e| e.0)?;
        let mut champion = ChampionState::default();
        champion.observe(1, &session.view());
        champion.step(0., 1.).map_err(str::to_owned)?;
        let validation = renderer
            .device
            .push_error_scope(wgpu::ErrorFilter::Validation);
        let mut measurements = Vec::new();
        let mut menu = Menu::default();
        let mut text = TextCache::menu();
        for scale in [1., 1.5, 2.] {
            let extent = [(960. * scale) as u32, (720. * scale) as u32];
            let panel = champion.panel(&session.view(), [960., 720.]);
            let mut input = PanelInputState::default();
            let mut edit = MenuInputState::default();
            input.sync(&panel, &mut edit);
            edit.apply(EditIntent::KeyText("Thanks for a great game!".into()));

            let mut scroll = std::collections::BTreeMap::new();
            let mut view = menu_specimen::view(0)?;
            // Authored returned-score data; the fresh-login specimen has none.
            view.last_life = slither_client::start_page::LastLifeView {
                label: view.wording.final_length,
                number: "1234".into(),
                suffix: "!",
                label_alpha: 0.45,
            };
            view.ordinary_controls_visible = false;
            view.champion_pending = false;
            view.tips_visible = false;
            let layout = menu_specimen::layout(&mut text, &view, extent, scale)?;
            let picture = menu.prepare_with_panel(
                &mut text,
                &view,
                &layout,
                extent,
                scale,
                None,
                None,
                Some(slither_wgpu_slice::menu::PanelPaint {
                    panel: &panel,
                    input: &input,
                    edit: &edit,
                    scroll: &mut scroll,
                    theme: MenuTheme::default(),
                }),
            )?;
            capture(
                &mut renderer,
                &picture,
                &output.join(format!("champion-scale-{scale}.png")),
            )?;
            measurements.push(serde_json::json!({"scale":scale,"draws":picture.frame.draws.len(),"resources":picture.resources.len(),"bytes":picture.resources.iter().map(|r|r.image.pixels.len()).sum::<usize>()}));
            edit.apply(EditIntent::SelectAll);
            edit.apply(EditIntent::KeyText("Wide message ".repeat(12)));
            let long = menu.prepare_with_panel(
                &mut text,
                &view,
                &layout,
                extent,
                scale,
                None,
                None,
                Some(slither_wgpu_slice::menu::PanelPaint {
                    panel: &panel,
                    input: &input,
                    edit: &edit,
                    scroll: &mut scroll,
                    theme: MenuTheme::default(),
                }),
            )?;
            capture(
                &mut renderer,
                &long,
                &output.join(format!("champion-scroll-scale-{scale}.png")),
            )?;
            view.ordinary_controls_visible = true;
            let layout = menu_specimen::layout(&mut text, &view, extent, scale)?;
            let login = menu.prepare(&mut text, &view, &layout, extent, scale, None)?;
            capture(
                &mut renderer,
                &login,
                &output.join(format!("login-scale-{scale}.png")),
            )?;
        }
        std::fs::write(
            output.join("champion.json"),
            serde_json::to_vec_pretty(&measurements).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        if let Some(e) = validation.pop().await {
            return Err(e.to_string());
        }
        return Ok(());
    }
    if std::env::args().nth(2).as_deref() == Some("--stroke") {
        return stroke_specimens(output, &mut renderer);
    }
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
            let p = menu.prepare(&mut text, &view, &layout, extent, dpr, None)?;
            capture(
                &mut renderer,
                &p,
                &output.join(format!("native-{name}-dpr-{dpr}.png")),
            )?;
        }
    }
    let view = menu_specimen::view(0)?;
    let mut images = Images::default();
    images.add("menu/skin.png", &synthetic_icon([90, 180, 120, 255])?)?;
    let mut art = Cosmetics::from_images(images, None)?;
    let layout = menu_specimen::layout(&mut text, &view, [1280, 720], 1.)?;
    let old = menu.prepare(&mut text, &view, &layout, [1280, 720], 1., Some(&art))?;
    capture(
        &mut renderer,
        &old,
        &output.join("native-synthetic-art.png"),
    )?;
    art.replace_menu_art(MenuArt::Skin, &synthetic_icon([170, 90, 180, 255])?)?;
    let replacement = menu.prepare(&mut text, &view, &layout, [1280, 720], 1., Some(&art))?;
    capture(
        &mut renderer,
        &replacement,
        &output.join("native-replacement.png"),
    )?;
    let layout = menu_specimen::layout(&mut text, &view, [960, 540], 1.)?;
    let resized = menu.prepare(&mut text, &view, &layout, [960, 540], 1., None)?;
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
        &menu.prepare(&mut text, &view, &layout, [640, 360], 1., None)?,
        &output.join("native-short-640x360.png"),
    )?;
    if let Some(error) = validation.pop().await {
        return Err(error.to_string());
    }
    Ok(())
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
            let mut picture = login.prepare_host(extent, 1., true, None)?;
            if let Some(backdrop) = picture.backdrop {
                picture.frame.draws.insert(0, backdrop);
            }
            renderer.sync_resources(&picture.resources)?;
            renderer.render_overlay(&picture.frame, &target)?;
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
) -> Result<(MenuFrame, [usize; 3])> {
    let logical = extent.map(|n| f64::from(n) / dpr);
    let art = match art {
        Some(art) => {
            login.studio.sync_art(logical, Some(&mut *art))?;
            Some(&*art)
        }
        None => None,
    };
    let mut picture = login.prepare_host(extent, dpr, admission, art)?;
    let mut preview = [0; 3];
    if login.studio.state.visible() {
        let (frame, resources) = login.studio.prepare_preview(extent, dpr, art)?;
        preview = [
            frame.draws.len(),
            resources.len(),
            resources.iter().map(|r| r.image.pixels.len()).sum(),
        ];
        picture.frame.draws.extend(frame.draws);
        picture.resources.extend(resources);
    }
    Ok((picture, preview))
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
                    let (picture, [preview, slots, bytes]) =
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
        let (picture, _) = studio_picture(&mut login, [960, 540], 1., false, None)?;
        let path = output.join(format!("transition-{time:03}ms.png"));
        capture_layered(renderer, &picture, &mut login, &path)?;
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
    let (picture, _) = studio_picture(&mut narrow, [360, 640], 1., false, None)?;
    capture_layered(
        renderer,
        &picture,
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
            let (picture, _) = studio_picture(&mut login, extent, scale, false, None)?;
            capture_layered(
                renderer,
                &picture,
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
        let (picture, _) = studio_picture(&mut login, extent, dpr, true, None)?;
        capture_layered(
            renderer,
            &picture,
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
            let (picture, _) = studio_picture(&mut login, extent, dpr, true, None)?;
            capture_layered(
                renderer,
                &picture,
                &mut login,
                &output.join(format!("studio-{name}-{dpr}x.png")),
            )?;
            login.studio.presented();
        }
    }
    std::fs::write(output.join("README.txt"),"Authored offline specimens; no windows, sockets or official art. Synthetic-ready bulbs and decorations are placeholder images.\nThe studio is egui, composed as the desktop host presents it: page, egui login and studio, then the preview above them.\nLogical 960x540 at DPR 1, 1.5, 2. Numeric ledger additionally covers DPR 3, 4, 5.\nStrip: row 1 entry at 0,58,115,230 ms; row 2 return at 288,345,459,460 ms; Cancel at 230 ms.\nlogin/studio-entering/studio-open: 960x720 logical over the settled login at DPR 1, 1.5, 2.\nOwner appearance, physical input, display pacing and server acceptance are unverified.\n").map_err(|e|e.to_string())?;
    Ok(())
}
/// Authored string/raster probes and the production menu at frozen core times.
/// Output stays external; optional private reference composition is a separate
/// local operation and never part of this example's inputs or repository data.
fn stroke_specimens(output: &Path, renderer: &mut Renderer) -> Result<()> {
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
        let layout = wordmark::layout(wordmark::DEFAULT_TEXT, holder, 1920., |_| {
            Err("unexpected fallback".into())
        })?;
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
        let layout = wordmark::layout(value, holder, 1920., |_| Err("unexpected fallback".into()))?;
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
    let mut menu = Menu::default();
    let mut text = TextCache::menu();
    let mut state = StartMenuState::new(MenuLocale::English, true, MenuQuality::High);
    let mut last_ms = 0;
    for ms in [0, 512, 1025, 2048, 2050, 3075, 4100, 5125, 6150, 7175] {
        for now in ((last_ms + 16)..ms).step_by(16) {
            state.step(f64::from(now), false).map_err(str::to_owned)?;
        }
        state.step(f64::from(ms), false).map_err(str::to_owned)?;
        last_ms = ms;
        let view = state.view(true);
        let layout = menu_specimen::layout(&mut text, &view, [1920, 1080], 1.)?;
        let p = menu.prepare(&mut text, &view, &layout, [1920, 1080], 1., None)?;
        capture(renderer, &p, &output.join(format!("reveal-{ms:05}ms.png")))?;
    }
    for dpr in [1., 2.] {
        let extent = [(1920. * dpr) as u32, (1080. * dpr) as u32];
        let view = state.view(true);
        let layout = menu_specimen::layout(&mut text, &view, extent, dpr)?;
        let p = menu.prepare(&mut text, &view, &layout, extent, dpr, None)?;
        capture(
            renderer,
            &p,
            &output.join(format!("settled-1920x1080-dpr-{dpr}.png")),
        )?;
        let mut cached_ms = Vec::new();
        for _ in 0..6 {
            let now = std::time::Instant::now();
            let cached = menu.prepare(&mut text, &view, &layout, extent, dpr, None)?;
            cached_ms.push(now.elapsed().as_secs_f64() * 1000.);
            if cached
                .frame
                .draws
                .iter()
                .map(|d| d.asset)
                .collect::<Vec<_>>()
                != p.frame.draws.iter().map(|d| d.asset).collect::<Vec<_>>()
            {
                return Err("settled resources changed".into());
            }
            for resource in &p.resources {
                let retained = cached
                    .resources
                    .iter()
                    .find(|r| r.asset == resource.asset)
                    .ok_or("settled resource missing")?;
                if !std::sync::Arc::ptr_eq(&resource.image, &retained.image) {
                    return Err("settled raster was copied".into());
                }
            }
        }
        timings.push(serde_json::json!({"dpr":dpr, "cachedMenuMilliseconds":cached_ms,
            "draws":p.frame.draws.len(), "pixelBytes":p.resources.iter().map(|r|r.image.pixels.len()).sum::<usize>()}));
    }
    // A ten-fps CPU raster sequence preserves the shared clock's timing for an
    // external motion strip/video. Full GPU menus above remain separate proof.
    let motion = output.join("motion");
    std::fs::create_dir(&motion).map_err(|e| e.to_string())?;
    let geometry = wordmark::layout(wordmark::DEFAULT_TEXT, holder, 1920., |_| unreachable!())?;
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
fn capture_settings(
    renderer: &mut Renderer,
    picture: &MenuFrame,
    login: &mut slither_wgpu_slice::login_native::NativeLogin,
    path: &Path,
) -> Result<()> {
    let mut frame = picture.frame.clone();
    if let Some(backdrop) = &picture.backdrop {
        frame.draws.insert(0, backdrop.clone());
    }
    let pixels = readback_with(renderer, &frame, &picture.resources, None, |r, v| {
        login.egui.paint(r, v)
    })?;
    image::save_buffer(
        path,
        &pixels,
        frame.extent[0],
        frame.extent[1],
        image::ColorType::Rgba8,
    )
    .map_err(|e| e.to_string())
}

/// The desktop host's layer order: page, world cover, egui, then the top
/// layer (the studio preview) above it. These login-only specimens have no
/// game world between the page and its cover.
#[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
fn capture_layered(
    renderer: &mut Renderer,
    picture: &MenuFrame,
    login: &mut slither_wgpu_slice::login_native::NativeLogin,
    path: &Path,
) -> Result<()> {
    let mut frame = picture.frame.clone();
    let top = Frame {
        draws: std::mem::take(&mut frame.draws),
        snakes: Vec::new(),
        ..frame.clone()
    };
    frame
        .draws
        .extend(picture.backdrop.iter().chain(&picture.world_cover).cloned());
    let mut layer = Ok(());
    let pixels = readback_with(renderer, &frame, &picture.resources, None, |r, v| {
        login.egui.paint(r, v);
        layer = r.render_overlay(&top, v);
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

#[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
fn login_specimens(output: &Path, renderer: &mut Renderer) -> Result<()> {
    use slither_wgpu_slice::login_native::NativeLogin;
    for (extent, dpr) in [([960, 540], 1.), ([360, 640], 1.), ([720, 1280], 2.)] {
        let mut login = NativeLogin::new("Player")?;
        for now in (0..=9600).step_by(16) {
            login.step(now as f64, true, false)?;
        }
        login.prepare_host(extent, dpr, true, None)?;
        let after = login.prepare_host(extent, dpr, true, None)?;
        // The retained toolkit's authored specimen remains the baseline for
        // screens awaiting their port. Both sides use the same shared projection.
        let before = Menu::default().prepare(
            &mut TextCache::menu(),
            &login.state.view(true),
            &login.layout,
            extent,
            dpr,
            None,
        )?;
        capture(
            renderer,
            &before,
            &output.join(format!("login-before-{}x{}.png", extent[0], extent[1])),
        )?;
        capture_settings(
            renderer,
            &after,
            &mut login,
            &output.join(format!("login-after-{}x{}.png", extent[0], extent[1])),
        )?;
    }
    Ok(())
}
