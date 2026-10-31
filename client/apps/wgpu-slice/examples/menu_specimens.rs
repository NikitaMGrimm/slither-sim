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
    if std::env::args().nth(2).as_deref() == Some("--studio-panel") {
        let reference = std::env::args_os().nth(3);
        return studio_panel::specimens(&mut renderer, output, reference.as_deref().map(Path::new))
            .await;
    }
    #[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
    if std::env::args().nth(2).as_deref() == Some("--settings") {
        return settings_specimens(output, &mut renderer).await;
    }
    #[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
    if std::env::args().nth(2).as_deref() == Some("--builder") {
        return builder::specimens(&mut renderer, output);
    }
    #[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
    if std::env::args().nth(2).as_deref() == Some("--server-picker") {
        use slither_client::{server_directory::Server, server_picker::ServerPreference};
        use slither_wgpu_slice::login_native::NativeLogin;
        use std::{collections::BTreeMap, net::SocketAddr};
        let validation = renderer
            .device
            .push_error_scope(wgpu::ErrorFilter::Validation);
        let servers = (1..=2000)
            .map(|id| Server {
                id,
                cluster: id % 5,
                address: SocketAddr::from(([192, 0, 2, 1], id)),
                population: 40,
                active: true,
            })
            .collect::<Vec<_>>();
        let pings = BTreeMap::from([(0, 18), (1, 67), (2, 130), (3, 285)]);
        let mut measurements = Vec::new();
        // One resource owner across DPR changes, like the actual native host.
        let mut login = NativeLogin::new("")?;
        for now in (0..=9600).step_by(16) {
            login.step(now as f64, true, false)?;
        }
        let mut now = 9600.;
        for dpr in [1., 1.5, 2.] {
            let extent = [(960. * dpr) as u32, (720. * dpr) as u32];
            login.picker.policy.close();
            for _ in 0..40 {
                now += 16.;
                login.step(now, true, false)?;
            }
            let picture = login.prepare_host(extent, dpr, true, None)?;
            capture(
                &mut renderer,
                &picture,
                &output.join(format!("server-login-{dpr}x.png")),
            )?;
            login
                .picker
                .policy
                .replace(&servers, &pings)
                .map_err(str::to_owned)?;
            login.picker.policy.selection = ServerPreference(Some(25));
            login.picker.open();
            for _ in 0..40 {
                now += 16.;
                login.step(now, true, false)?;
            }
            for (name, offset) in [("picker", 0), ("scrolled", 1994)] {
                login.picker.policy.scroll(offset);
                login.picker.sync();
                let picture = login.prepare_host(extent, dpr, true, None)?;
                capture(
                    &mut renderer,
                    &picture,
                    &output.join(format!("server-{name}-{dpr}x.png")),
                )?;
                measurements.push(serde_json::json!({"dpr":dpr,"state":name,"directory_rows":2000,"draws":picture.frame.draws.len(),"resources":picture.resources.len(),"bytes":picture.resources.iter().map(|r|r.image.pixels.len()).sum::<usize>()}));
            }
        }
        // Retain the same native menu across the reported DPR-3 resize trigger.
        for logical in [[960., 540.], [960., 720.]] {
            let dpr = 3.;
            let extent = logical.map(|v| (v * dpr) as u32);
            let picture = login.prepare_host(extent, dpr, true, None)?;
            capture(
                &mut renderer,
                &picture,
                &output.join(format!(
                    "server-resize-3x-{}x{}.png",
                    logical[0], logical[1]
                )),
            )?;
            measurements.push(serde_json::json!({"dpr":dpr,"logical":logical,"state":"resize","directory_rows":2000,"draws":picture.frame.draws.len(),"resources":picture.resources.len(),"bytes":picture.resources.iter().map(|r|r.image.pixels.len()).sum::<usize>()}));
        }
        std::fs::write(
            output.join("server-picker.json"),
            serde_json::to_vec_pretty(&measurements).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        if let Some(error) = validation.pop().await {
            return Err(error.to_string());
        }
        return Ok(());
    }
    #[cfg(not(target_os = "linux"))]
    if std::env::args().nth(2).as_deref() == Some("--studio") {
        return studio_specimens(&mut renderer, output);
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
    #[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
    if std::env::args().nth(2).as_deref() == Some("--login") {
        use slither_client::menu_layout::MenuElement as E;
        use slither_wgpu_slice::login_native::NativeLogin;
        let art = std::env::args_os()
            .nth(3)
            .map(|dir| Cosmetics::directory(Path::new(&dir), None))
            .transpose()?;
        let mut login = NativeLogin::new("")?;
        for now in (0..=9600).step_by(16) {
            login.step(now as f64, true, false)?;
        }
        let mut measurements = Vec::new();
        let mut text = TextCache::default();
        let mut text_widths = Vec::new();
        for (label, size, bold) in [
            ("slither", 40., true),
            ("Nickname", 15., false),
            ("Play", 20., true),
            ("Change Skin", 14., false),
            ("High quality", 14., false),
            ("When longer, hold the mouse for a speed boost!", 14., false),
        ] {
            let style = slither_wgpu_slice::text::TextStyle {
                size,
                weight: if bold {
                    slither_wgpu_slice::text::Weight::Bold
                } else {
                    slither_wgpu_slice::text::Weight::Normal
                },
                family: slither_wgpu_slice::text::Family::Arial,
                slant: slither_wgpu_slice::text::Slant::Normal,
                color: [255; 3],
            };
            text_widths.push(serde_json::json!({"text":label,"size":size,"bold":bold,"advance":text.measure_panel(label,style)?.advance}));
        }
        std::fs::write(
            output.join("text-widths.json"),
            serde_json::to_vec_pretty(&text_widths).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        for (logical, dpr) in [
            ([960, 540], 1.),
            ([1920, 1080], 1.),
            ([1280, 720], 1.),
            ([640, 360], 1.),
            ([960, 540], 1.5),
            ([960, 540], 2.),
            ([1920, 1080], 2.),
        ] {
            let extent = logical.map(|n| (f64::from(n) * dpr) as u32);
            let picture = login.prepare_host(extent, dpr, true, art.as_ref())?;
            capture(
                &mut renderer,
                &picture,
                &output.join(format!(
                    "native-{}x{}-dpr-{dpr}.png",
                    logical[0], logical[1]
                )),
            )?;
            let rects: Vec<_> = [
                E::Logo,
                E::NicknameHolder,
                E::Nickname,
                E::Play,
                E::Skin,
                E::Quality,
                E::Spinner,
            ]
            .into_iter()
            .map(|e| {
                let r = login.layout.viewport_rect(e);
                vec![r.x, r.y, r.width, r.height]
            })
            .collect();
            measurements.push(serde_json::json!({"extent":extent,"logical":logical,"dpr":dpr,"backing":picture.frame.extent,"rects":rects,"clock":9600,"draws":picture.frame.draws.len() + usize::from(picture.backdrop.is_some()),"pixelBytes":picture.resources.iter().map(|r|r.image.pixels.len()).sum::<usize>()}));
        }
        for (name, time) in [("on", 9600.), ("off", 10130.)] {
            if name == "on" {
                login.focus(slither_client::login_controls::NICKNAME);
            }
            login.step(time, true, false)?;
            let picture = login.prepare_host([960, 540], 1., true, art.as_ref())?;
            capture(
                &mut renderer,
                &picture,
                &output.join(format!("caret-{name}.png")),
            )?;
        }
        login.focus(slither_client::login_controls::PLAY);
        login.activate(true);
        for now in (10146..=10930).step_by(16) {
            login.step(f64::from(now), true, false)?;
        }
        let picture = login.prepare_host([960, 540], 1., true, art.as_ref())?;
        capture(&mut renderer, &picture, &output.join("native-spinner.png"))?;
        for without_icons in [false, true] {
            // Each independent producer starts its own material revision sequence.
            let mut intro_renderer =
                Renderer::new(&instance(), None, wgpu::TextureFormat::Rgba8Unorm).await?;
            let mut intro = NativeLogin::new("")?;
            for now in (0..=768).step_by(16) {
                intro.step(f64::from(now), true, false)?;
                if [128, 256, 512, 768].contains(&now) {
                    let picture = intro.prepare_host(
                        [960, 540],
                        1.,
                        true,
                        if without_icons { None } else { art.as_ref() },
                    )?;
                    capture(
                        &mut intro_renderer,
                        &picture,
                        &output.join(format!("reveal-{now}-without-icons-{without_icons}.png")),
                    )?;
                }
            }
        }
        std::fs::write(
            output.join("native.json"),
            serde_json::to_vec_pretty(&measurements).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        return Ok(());
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
#[cfg(not(target_os = "linux"))]
fn studio_specimens(renderer: &mut Renderer, output: &Path) -> Result<()> {
    use slither_client::{
        presentation::{FaceGeometry, resolve_skin},
        skin_studio::*,
    };
    use slither_wgpu_slice::{
        login_native::NativeLogin, menu_storage::MenuStorage, skin_studio_native::SkinStudioNative,
    };
    let png = synthetic_icon([180, 100, 210, 255])?;
    let full = Cosmetics::load(
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
    let mut ledger = vec!["dpr,skin,art,clock_ms,control_draws,preview_draws,menu_slots,menu_bytes,text_slots,text_bytes,preview_slots,preview_bytes".to_owned()];
    let mut menu = Menu::default();
    let mut text = TextCache::menu();
    let mut controls = slither_client::menu_controls::PanelInputState::default();
    let mut edit = slither_client::menu_input::MenuInputState::default();
    let mut scroll = Default::default();
    let mut studio = SkinStudioNative::new(AppearanceRecord::default(), MenuStorage::at(None))?;
    for scale in [1., 1.5, 2., 3., 4., 5.] {
        let extent = [(960. * scale) as u32, (540. * scale) as u32];
        for skin in 0..=65 {
            for (art, sources) in [("fallback", None), ("synthetic-ready", Some(&full))] {
                studio.state = StudioState::new(AppearanceRecord {
                    builtin: skin,
                    ..Default::default()
                })
                .map_err(str::to_owned)?;
                studio.step(0.);
                studio.open(true);
                for time in [0., 230., 355., 730.] {
                    studio.step(time);
                    let (preview, resources) = studio.prepare_preview(extent, scale, sources)?;
                    let panel = studio.state.panel([960., 540.], &studio.status);
                    controls.sync(&panel, &mut edit);
                    let mut picture = menu.prepare_panel(
                        &mut text,
                        &panel,
                        &controls,
                        &edit,
                        &mut scroll,
                        Default::default(),
                        extent,
                        scale,
                    )?;
                    let menu_slots = picture.resources.len();
                    let menu_bytes: usize =
                        picture.resources.iter().map(|r| r.image.pixels.len()).sum();
                    let text_sources: Vec<_> = picture
                        .resources
                        .iter()
                        .filter(|r| r.asset.handle >> 60 == 8)
                        .collect();
                    let text_bytes: usize = text_sources.iter().map(|r| r.image.pixels.len()).sum();
                    ledger.push(format!("{scale},{skin},{art},{time},{},{},{menu_slots},{menu_bytes},{},{text_bytes},{},{}",
                        picture.frame.draws.len(),preview.draws.len(),text_sources.len(),resources.len(),resources.iter().map(|r|r.image.pixels.len()).sum::<usize>()));
                    if picture.frame.draws.len() > 384
                        || preview.draws.len() > 1024
                        || menu_slots > 40
                        || menu_bytes > 8 * 1024 * 1024
                        || text_sources.len() > 20
                        || text_bytes > 2 * 1024 * 1024
                    {
                        return Err("Studio working allocation exceeded".into());
                    }
                    let requested = resolve_skin(skin, &[], false).map_err(|e| e.to_string())?;
                    let special =
                        requested.antenna.is_some() || requested.face != FaceGeometry::default();
                    if scale <= 2. && time == 730. && (skin == 0 || skin == 65 || special) {
                        let page = MenuFrame::world_crossfade(extent, scale, 0.)?;
                        picture.backdrop = page.backdrop;
                        picture.resources.extend(page.resources);
                        picture.frame.draws.extend(preview.draws);
                        picture.resources.extend(resources);
                        capture(
                            renderer,
                            &picture,
                            &output.join(format!("studio-{skin:02}-{art}-{scale}x.png")),
                        )?;
                    }
                    studio.presented();
                }
            }
        }
    }
    std::fs::write(output.join("dpr-ledger.csv"), ledger.join("\n") + "\n")
        .map_err(|e| e.to_string())?;
    // New fixture owners restart their handle allocators. Retire the previous
    // fixture before reusing that namespace, as a new host would.
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
        let mut picture = login.prepare_host([960, 540], 1., false, None)?;
        if login.studio.state.visible() {
            let (preview, resources) = login.studio.prepare_preview([960, 540], 1., None)?;
            picture.frame.draws.extend(preview.draws);
            picture.resources.extend(resources);
        }
        let path = output.join(format!("transition-{time:03}ms.png"));
        capture(renderer, &picture, &path)?;
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
    let mut picture = narrow.prepare_host([360, 640], 1., false, None)?;
    let (preview, resources) = narrow.studio.prepare_preview([360, 640], 1., None)?;
    picture.frame.draws.extend(preview.draws);
    picture.resources.extend(resources);
    capture(
        renderer,
        &picture,
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
            let mut picture = login.prepare_host(extent, scale, false, None)?;
            let (preview, resources) = login.studio.prepare_preview(extent, scale, None)?;
            picture.frame.draws.extend(preview.draws);
            picture.resources.extend(resources);
            capture(
                renderer,
                &picture,
                &output.join(format!("studio-{name}-status-{scale}x.png")),
            )?;
        }
    }
    std::fs::write(output.join("README.txt"),"Authored offline specimens; no windows, sockets or official art. Synthetic-ready bulbs and decorations are placeholder images.\nLogical 960x540 at DPR 1, 1.5, 2. Numeric ledger additionally covers DPR 3, 4, 5.\nStrip: row 1 entry at 0,58,115,230 ms; row 2 return at 288,345,459,460 ms; Cancel at 230 ms.\nOwner appearance, physical input, display pacing and server acceptance are unverified.\n").map_err(|e|e.to_string())?;
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

#[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
async fn settings_specimens(output: &Path, renderer: &mut Renderer) -> Result<()> {
    use slither_client::{
        login_controls::{NICKNAME, SETTINGS},
        menu_controls::ControlKind,
        menu_preferences::MenuQuality,
        settings::{DONE, QUALITY, RESOLUTION, WorldResolution},
    };
    use slither_wgpu_slice::login_native::NativeLogin;
    let validation = renderer
        .device
        .push_error_scope(wgpu::ErrorFilter::Validation);
    let mut login = NativeLogin::new("Player")?;
    for now in (0..=9600).step_by(16) {
        login.step(now as f64, true, false)?;
    }
    let mut measurements = Vec::new();
    let mut now = 9600.;
    for (logical, dpr) in [
        ([960., 540.], 1.),
        ([960., 540.], 1.5),
        ([960., 540.], 2.),
        ([960., 540.], 3.),
        ([960., 540.], 4.),
        ([960., 540.], 5.),
        ([960., 540.], 6.),
        ([960., 540.], 7.),
        ([960., 540.], 8.),
        ([360., 640.], 1.),
        ([360., 640.], 2.),
        ([360., 640.], 5.),
    ] {
        let extent = logical.map(|v| (v * dpr) as u32);
        login.close_settings(true);
        login.focus(NICKNAME);
        assert_eq!(login.controls.focus(), Some(NICKNAME));
        let outgoing = login.prepare_host(extent, dpr, true, None)?;
        capture(
            renderer,
            &outgoing,
            &output.join(format!("login-{}x{}-{dpr}x.png", logical[0], logical[1])),
        )?;
        login.focus(SETTINGS);
        login.open_settings(true);
        for state in [
            "high-focus",
            "high-pressed",
            "not-saved-pressed",
            "low-focus",
            "not-saved",
            "done-focus",
            "disabled",
            "native-focus",
            "native-unavailable-not-saved",
        ] {
            login.focus(RESOLUTION);
            if (login.state.settings.preferences().resolution == WorldResolution::Native)
                != state.starts_with("native")
            {
                login.activate(true);
                now += 16.;
                login.step(now, true, false)?;
            }
            login
                .state
                .settings
                .native_available(state != "native-unavailable-not-saved");
            let high = matches!(state, "high-focus" | "high-pressed");
            login.focus(QUALITY);
            if (login.state.view(true).quality == MenuQuality::High) != high {
                login.activate(true);
                now += 16.;
                login.step(now, true, false)?;
            }
            login.state.settings.storage_result(!matches!(
                state,
                "not-saved" | "not-saved-pressed" | "native-unavailable-not-saved"
            ));
            login.controls.set_progress(
                QUALITY,
                if matches!(state, "high-pressed" | "not-saved-pressed") {
                    [1., 1.]
                } else {
                    [0., 0.]
                },
            );
            if matches!(state, "done-focus" | "not-saved-pressed") {
                login.focus(DONE);
            }
            login.controls.set_progress(
                RESOLUTION,
                if state == "native-unavailable-not-saved" {
                    [1., 1.]
                } else {
                    [0., 0.]
                },
            );
            if state.starts_with("native") {
                login.focus(RESOLUTION);
            }
            let mut picture = login.prepare_host(extent, dpr, true, None)?;
            if state == "disabled" {
                // Authored shared-control specimen; v1 has no unavailable Quality mode.
                let mut panel = login.panel.clone();
                panel
                    .controls
                    .iter_mut()
                    .find(|c| c.id == QUALITY)
                    .unwrap()
                    .enabled = false;
                picture = Menu::default().prepare_panel(
                    &mut TextCache::menu(),
                    &panel,
                    &login.controls,
                    &login.input.state,
                    &mut Default::default(),
                    Default::default(),
                    extent,
                    dpr,
                )?;
                let page = MenuFrame::world_crossfade(extent, dpr, 0.)?;
                picture.backdrop = page.backdrop;
                picture.resources.extend(page.resources);
                assert!(matches!(
                    panel
                        .controls
                        .iter()
                        .find(|c| c.id == QUALITY)
                        .unwrap()
                        .kind,
                    ControlKind::Toggle { checked: false, .. }
                ));
            }
            let bytes = picture
                .resources
                .iter()
                .map(|r| r.image.pixels.len())
                .sum::<usize>();
            if picture.resources.len() > 32
                || bytes > 4 * 1024 * 1024
                || picture.frame.draws.len() > if dpr <= 2. { 64 } else { 160 }
            {
                return Err(format!(
                    "Settings compactness regression: {logical:?} {dpr} {state}: {} slots, {bytes} bytes, {} quads",
                    picture.resources.len(),
                    picture.frame.draws.len()
                ));
            }
            let file = format!("settings-{state}-{}x{}-{dpr}x.png", logical[0], logical[1]);
            // The synthetic disabled panel uses a fresh painter/handle namespace.
            // Retire the preceding upload set before and after this isolated case.
            if state == "disabled" {
                renderer.sync_resources(&[])?;
            }
            capture(renderer, &picture, &output.join(&file))?;
            measurements.push(serde_json::json!({"file":file,"logical":logical,"dpr":dpr,"state":state,"producer_quads":picture.frame.draws.len(),"gpu_draws_including_page":renderer.draw_call_count(),"slots":picture.resources.len(),"bytes":bytes,"max_tile_axis":picture.resources.iter().map(|r|r.asset.width.max(r.asset.height)).max(),"retained_outgoing_bytes":outgoing.resources.iter().map(|r|r.image.pixels.len()).sum::<usize>(),"focus":format!("{:?}",login.controls.focus())}));
            if state == "disabled" {
                renderer.sync_resources(&[])?;
            }
        }
        outgoing.frame.validate(
            &outgoing
                .resources
                .iter()
                .map(|r| (r.asset.into(), r.asset))
                .collect(),
        )?;
        login.close_settings(true);
        assert_eq!(login.controls.focus(), Some(SETTINGS));
        let returned = login.prepare_host(extent, dpr, true, None)?;
        capture(
            renderer,
            &returned,
            &output.join(format!("return-{}x{}-{dpr}x.png", logical[0], logical[1])),
        )?;
    }
    let revision = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|e| e.to_string())?;
    let fonts = [slither_client::menu_theme::ControlStyle::Heading, slither_client::menu_theme::ControlStyle::HelpText, slither_client::menu_theme::ControlStyle::PrimaryButton].map(|role| serde_json::json!({"role":format!("{role:?}"),"face":TextCache::native_face(slither_client::menu_theme::MenuTheme::default().text_style(role))}));
    let manifest = serde_json::json!({"revision":String::from_utf8_lossy(&revision.stdout).trim(),"fixture":"authored synthetic; no game servers or external art","fonts":fonts,"panel_at_960x540":[200,70,560,400],"settings_record":"2:<0|1>:<original|native>","measurements":measurements,"limits":{"slots":32,"bytes":4*1024*1024,"producer_quads":160,"producer_quads_dpr_1_to_2":64,"menu_raster_density_cap":5},"unverified":"physical input, visible-window behavior, owner appearance/feel and live servers"});
    std::fs::write(
        output.join("settings.json"),
        serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
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
#[path = "menu_specimens/studio_panel.rs"]
mod studio_panel;
