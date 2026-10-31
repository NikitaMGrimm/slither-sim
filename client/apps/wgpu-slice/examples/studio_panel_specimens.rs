//! Finite authored native comparison; no window, transport or external art.
#[cfg(any(target_arch = "wasm32", target_os = "linux"))]
fn main() {
    eprintln!("Native studio specimens require the Windows text adapter");
}
#[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
fn main() {
    if let Err(e) = pollster::block_on(native::run()) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

#[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
mod native {
    use image::{
        ImageEncoder,
        codecs::png::{CompressionType, FilterType, PngEncoder},
    };
    use slither_client::{
        menu_controls::{ControlKind, PanelInputState},
        menu_input::MenuInputState,
        menu_layout::MenuRect,
        skin_studio::*,
    };
    use slither_wgpu_slice::{
        Result,
        gpu::{Renderer, instance},
        login_native::NativeLogin,
        menu::{Menu, MenuFrame},
        menu_counts::MenuComponent,
        text::TextCache,
    };
    use std::{collections::BTreeMap, fs, path::Path, sync::mpsc, time::Duration};

    fn pixels(renderer: &mut Renderer, menu: &MenuFrame) -> Result<Vec<u8>> {
        let mut frame = menu.frame.clone();
        if let Some(cover) = &menu.world_cover {
            frame.draws.insert(0, cover.clone());
        }
        if let Some(page) = &menu.backdrop {
            frame.draws.insert(0, page.clone());
        }
        renderer.sync_resources(&menu.resources)?;
        let [w, h] = frame.extent;
        let target = renderer.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("studio comparison"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        renderer.render_overlay(&frame, &target.create_view(&Default::default()))?;
        let row = (w * 4).div_ceil(256) * 256;
        let buffer = renderer.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("studio comparison readback"),
            size: u64::from(row) * u64::from(h),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = renderer.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
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
        let mut pixels = vec![0; w as usize * h as usize * 4];
        for (destination, source) in pixels
            .chunks_exact_mut(w as usize * 4)
            .zip(data.chunks_exact(row as usize))
        {
            destination.copy_from_slice(&source[..w as usize * 4]);
        }
        drop(data);
        buffer.unmap();
        Ok(pixels)
    }

    fn capture(
        renderer: &mut Renderer,
        menu: &MenuFrame,
        preview: usize,
        name: &str,
        out: &Path,
        reference: Option<&Path>,
        ledger: &mut String,
        differences: &mut String,
    ) -> Result<()> {
        let counts = menu.draw_counts(preview);
        if counts.components.total() + preview != menu.frame.draws.len() {
            return Err(format!("Attribution mismatch: {name}"));
        }
        if counts.admitted_menu() > 512 {
            return Err(format!("Menu quota: {name}"));
        }
        let components = MenuComponent::ALL
            .map(|c| counts.components.get(c).to_string())
            .join(",");
        ledger.push_str(&format!(
            "{name},{components},{},{},{},{},{},{}\n",
            counts.page,
            counts.cover,
            preview,
            counts.reported_menu(),
            counts.admitted_menu(),
            counts.combined()
        ));
        let rgba = pixels(renderer, menu)?;
        let [w, h] = menu.frame.extent;
        if let Some(reference) = reference {
            let before = image::open(reference.join(format!("{name}.png")))
                .map_err(|e| e.to_string())?
                .to_rgba8();
            if before.dimensions() != (w, h) {
                return Err("Comparison extent mismatch".into());
            }
            let mut delta = 0;
            let mut at = 0;
            let mut changed = 0u64;
            if before.as_raw() != &rgba {
                for (i, (a, b)) in before.as_raw().iter().zip(&rgba).enumerate() {
                    let d = a.abs_diff(*b);
                    if d > delta {
                        delta = d;
                        at = i;
                    }
                    if d != 0 {
                        changed += 1;
                    }
                }
            }
            differences.push_str(&format!(
                "{name},{delta},{},{},{},{changed}\n",
                at / 4 % w as usize,
                at / 4 / w as usize,
                at % 4
            ));
        }
        PngEncoder::new_with_quality(
            fs::File::create(out.join(format!("{name}.png"))).map_err(|e| e.to_string())?,
            CompressionType::Fast,
            FilterType::NoFilter,
        )
        .write_image(&rgba, w, h, image::ExtendedColorType::Rgba8)
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn run() -> Result<()> {
        let args: Vec<_> = std::env::args().collect();
        let out =
            Path::new(args.get(1).ok_or(
                "Expected new output directory, optionally followed by reference directory",
            )?);
        let reference = args.get(2).map(Path::new);
        fs::create_dir(out).map_err(|e| e.to_string())?;
        let mut renderer =
            Renderer::new(&instance(), None, wgpu::TextureFormat::Rgba8Unorm).await?;
        let validation = renderer
            .device
            .push_error_scope(wgpu::ErrorFilter::Validation);
        let mut ledger = String::from(
            "case,login_wordmark,login_controls,login_text,studio_shade,studio_surface,studio_chrome,studio_section,other,page,cover,preview,reported_menu,admitted_menu,combined\n",
        );
        let mut differences = String::from("case,max_channel_delta,x,y,channel,changed_channels\n");
        for dpr in [1., 1.25, 1.5, 2., 3., 4., 5., 6., 7., 8.] {
            for (layout, logical) in [
                ("wide", [960., 540.]),
                ("narrow", [360., 640.]),
                ("short", [960., 360.]),
            ] {
                let extent = logical.map(|n| (n * dpr) as u32);
                for builder in [false, true] {
                    renderer.sync_resources(&[])?;
                    let mut login = NativeLogin::new("Player")?;
                    for now in (0..=9600).step_by(16) {
                        login.step(now as f64, false, false)?;
                    }
                    login.prepare_host(extent, dpr, false, None)?;
                    login.open_studio();
                    if builder {
                        // Deliberately switch before entry finishes: the former 494 case.
                        login.studio.activate(BUILDER_TAB, "Player");
                        for n in 0..47 {
                            login.studio.state.edit_builder(|b| {
                                b.append(slither_client::skin_builder::PALETTES[n % 39]);
                            });
                        }
                    }
                    for time in [0., 1., 90., 115., 180., 229., 230., 410., 525., 639., 640.] {
                        login.step(9600. + time, false, false)?;
                        if time == 410. {
                            login.studio.state.cancel(10010.);
                            login.step(10010., false, false)?;
                        }
                        let mut menu = login.prepare_host(extent, dpr, false, None)?;
                        let preview = if login.studio.state.visible() {
                            let (frame, resources) =
                                login.studio.prepare_preview(extent, dpr, None)?;
                            let count = frame.draws.len();
                            menu.frame.draws.extend(frame.draws);
                            menu.resources.extend(resources);
                            count
                        } else {
                            0
                        };
                        let name = format!(
                            "{layout}-{}-{dpr}x-{time:03}",
                            if builder { "builder" } else { "builtin" }
                        );
                        capture(
                            &mut renderer,
                            &menu,
                            preview,
                            &name,
                            out,
                            reference,
                            &mut ledger,
                            &mut differences,
                        )?;
                        login.studio.presented();
                    }
                    // Explicit fractional clip cuts the surface/shadow; other controls stay intact.
                    renderer.sync_resources(&[])?;
                    let mut state = StudioState::new(AppearanceRecord {
                        builtin: 7,
                        ..Default::default()
                    })
                    .map_err(str::to_owned)?;
                    state.open(0., true);
                    state.step(230.);
                    if builder {
                        state.set_section(StudioSection::Builder, 230.);
                        state.step(410.);
                    }
                    let mut panel = state.panel(logical, "");
                    let bounds = StudioState::bounds(logical);
                    panel
                        .controls
                        .iter_mut()
                        .find(|c| matches!(c.kind, ControlKind::Surface))
                        .unwrap()
                        .clip = Some(MenuRect {
                        x: bounds.x + 3.25,
                        y: bounds.y + 5.5,
                        width: bounds.width - 19.75,
                        height: bounds.height - 11.25,
                    });
                    let menu = Menu::default().prepare_panel(
                        &mut TextCache::menu(),
                        &panel,
                        &PanelInputState::default(),
                        &MenuInputState::default(),
                        &mut BTreeMap::new(),
                        Default::default(),
                        extent,
                        dpr,
                    )?;
                    capture(
                        &mut renderer,
                        &menu,
                        0,
                        &format!(
                            "{layout}-{}-{dpr}x-clipped",
                            if builder { "builder" } else { "builtin" }
                        ),
                        out,
                        reference,
                        &mut ledger,
                        &mut differences,
                    )?;
                }
            }
            fs::write(out.join("draws.csv"), &ledger).map_err(|e| e.to_string())?;
            fs::write(out.join("pixels.csv"), &differences).map_err(|e| e.to_string())?;
            println!("Completed DPR {dpr}");
        }
        if let Some(error) = validation.pop().await {
            return Err(error.to_string());
        }
        println!("Completed {} cases", ledger.lines().count() - 1);
        Ok(())
    }
}
