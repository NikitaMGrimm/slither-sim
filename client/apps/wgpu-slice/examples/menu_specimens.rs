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
    renderer.sync_resources(&p.resources)?;
    let [width, height] = physical.unwrap_or(p.frame.extent);
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
        let [w, h] = p.frame.extent;
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
        renderer.render(&frame, &source)?;
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
        renderer.render_overlay(&frame, &texture.create_view(&Default::default()))?;
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
        frame.draws.len(),
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
                0,
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
                0,
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
            let login = menu.prepare(&mut text, &view, &layout, extent, scale, None, 0)?;
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
    #[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
    if std::env::args().nth(2).as_deref() == Some("--login") {
        use slither_client::menu_layout::MenuElement as E;
        use slither_wgpu_slice::login_native::NativeLogin;
        let art = std::env::args_os()
            .nth(3)
            .map(|dir| Cosmetics::directory(Path::new(&dir), None))
            .transpose()?;
        let wordmark = std::env::args().any(|a| a == "--wordmark");
        let mut login = NativeLogin::new("")?;
        login.set_wordmark(wordmark);
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
                login.focus(0);
            }
            login.step(time, true, false)?;
            let picture = login.prepare_host([960, 540], 1., true, art.as_ref())?;
            capture(
                &mut renderer,
                &picture,
                &output.join(format!("caret-{name}.png")),
            )?;
        }
        login.focus(1);
        login.activate(true);
        for now in (10146..=10930).step_by(16) {
            login.step(f64::from(now), true, false)?;
        }
        let picture = login.prepare_host([960, 540], 1., true, art.as_ref())?;
        capture(&mut renderer, &picture, &output.join("native-spinner.png"))?;
        for fallback in [false, true] {
            // Each independent producer starts its own material revision sequence.
            let mut intro_renderer =
                Renderer::new(&instance(), None, wgpu::TextureFormat::Rgba8Unorm).await?;
            let mut intro = NativeLogin::new("")?;
            intro.set_wordmark(wordmark);
            for now in (0..=768).step_by(16) {
                intro.step(f64::from(now), true, false)?;
                if [128, 256, 512, 768].contains(&now) {
                    let picture = intro.prepare_host(
                        [960, 540],
                        1.,
                        true,
                        if fallback { None } else { art.as_ref() },
                    )?;
                    capture(
                        &mut intro_renderer,
                        &picture,
                        &output.join(format!("reveal-{now}-fallback-{fallback}.png")),
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
