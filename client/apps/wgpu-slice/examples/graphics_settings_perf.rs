//! Authored offline graphics matrix: no window, socket, or external artwork.
#[path = "../tests/support/announcement_tape.rs"]
mod tape;
#[allow(dead_code)]
mod support {
    pub mod offscreen;
}
use slither_client::settings::WorldResolution::{Native, Original};
use slither_wgpu_slice::{Result, game::LocalGame, gpu, render, world_presenter::WorldPresenter};
use std::{collections::BTreeMap, fmt::Write, path::Path, time::Instant};

fn world() -> Result<LocalGame> {
    let mut g = tape::negotiating();
    let mut setup = tape::setup();
    setup[1..4].copy_from_slice(&tape::u24(12000));
    setup.extend([0, 0]);
    setup.extend(tape::u24(11500));
    let mut own = tape::snake(0);
    own[16..19].copy_from_slice(&tape::u24(60000));
    own[19..22].copy_from_slice(&tape::u24(60000));
    own[25..28].copy_from_slice(&tape::u24(59280));
    own[28..31].copy_from_slice(&tape::u24(60000));
    g.receive(g.generation(), &tape::bundle(&[setup, own]), 3.)?;
    tape::pictures(&mut g);
    // Exercise standard shaded skin sources and authored procedural cosmetics,
    // without supplying any external image or touching an art-pack directory.
    g.bank.cosmetics = Some(slither_wgpu_slice::cosmetics::Cosmetics::from_images(
        Default::default(),
        None,
    )?);
    let mut packets = Vec::new();
    for i in 0..24_u16 {
        let mut p = tape::snake((i % 7) as u8);
        p[1..3].copy_from_slice(&(100 + i).to_be_bytes());
        let (x, y) = (11850 + (i % 4) * 120, 11850 + (i / 4) * 220);
        p[16..19].copy_from_slice(&tape::u24(u32::from(x) * 5));
        p[19..22].copy_from_slice(&tape::u24(u32::from(y) * 5));
        p[25..28].copy_from_slice(&tape::u24(u32::from(x) * 5 - 720));
        p[28..31].copy_from_slice(&tape::u24(u32::from(y) * 5));
        packets.push(p);
    }
    for i in 0..2400_u16 {
        let (x, y) = (11700 + i % 60 * 12, 11700 + i / 60 * 15);
        packets.push(vec![
            b'F',
            (x / 100) as u8,
            (y / 100) as u8,
            (i % 9) as u8,
            ((x % 100) * 255 / 100) as u8,
            ((y % 100) * 255 / 100) as u8,
            20,
        ]);
    }
    packets.push(tape::prey());
    g.receive(g.generation(), &tape::bundle(&packets), 21.)?;
    tape::pictures(&mut g);
    // Update-clock spawn animation is independent of picture preparation. One
    // large timestamp jump is capped and leaves food nearly invisible.
    for step in 1..=240 {
        let now = 21. + f64::from(step) * 16.;
        let sent = tape::pump(&mut g, now);
        // In-memory pongs keep this authored warmup on the normal update clock.
        if sent.iter().any(|packet| packet.as_slice() == [251]) {
            g.receive(g.generation(), b"p", now)?;
            tape::pictures(&mut g);
        }
    }
    assert!(g.client().foods().foods().iter().all(|f| {
        let f = f.snapshot();
        f.fade == 1. && f.scale == 1.
    }));
    assert_eq!(g.phase(), slither_client::session_policy::Phase::Playing);
    Ok(g)
}

fn wait(r: &gpu::Renderer) -> Result<()> {
    r.device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(30)),
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// GPU timestamps surround the complete presenter in queue order. This span includes
/// CPU supply gaps between submissions; it is not an isolated shader/pass duration.
struct Timer {
    queries: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
}
impl Timer {
    fn new(r: &gpu::Renderer) -> Self {
        Self {
            queries: r.device.create_query_set(&wgpu::QuerySetDescriptor {
                label: None,
                ty: wgpu::QueryType::Timestamp,
                count: 2,
            }),
            resolve: r.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: 256,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            read: r.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: 16,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }),
        }
    }
    fn begin(&self, r: &gpu::Renderer) {
        let mut e = r.device.create_command_encoder(&Default::default());
        e.write_timestamp(&self.queries, 0);
        r.queue.submit([e.finish()]);
    }
    fn end(&self, r: &gpu::Renderer) -> Result<f64> {
        let mut e = r.device.create_command_encoder(&Default::default());
        e.write_timestamp(&self.queries, 1);
        e.resolve_query_set(&self.queries, 0..2, &self.resolve, 0);
        e.copy_buffer_to_buffer(&self.resolve, 0, &self.read, 0, 16);
        r.queue.submit([e.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        self.read
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |v| {
                let _ = tx.send(v);
            });
        wait(r)?;
        rx.recv()
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        let data = self
            .read
            .slice(..)
            .get_mapped_range()
            .map_err(|e| e.to_string())?;
        let start = u64::from_le_bytes(data[..8].try_into().unwrap());
        let end = u64::from_le_bytes(data[8..16].try_into().unwrap());
        let ns = end.checked_sub(start).ok_or("timestamp reversed")? as f64
            * f64::from(r.queue.get_timestamp_period());
        drop(data);
        self.read.unmap();
        Ok(ns)
    }
}

fn texel_scales(
    frame: &slither_wgpu_slice::frame::Frame,
    attachment: [u32; 2],
) -> serde_json::Value {
    let mut scales = BTreeMap::<u64, (u32, u32, f64, usize)>::new();
    for d in &frame.draws {
        if d.effective_alpha == 0. || d.repeat_uv.is_some() || d.corners.is_some() {
            continue;
        }
        let source = d.source.unwrap_or(slither_client::presentation::BodyRect {
            x: 0.,
            y: 0.,
            width: f64::from(d.asset.width),
            height: f64::from(d.asset.height),
        });
        let scale = (d.destination.width.abs() / source.width * f64::from(attachment[0])
            / f64::from(frame.extent[0]))
        .max(
            d.destination.height.abs() / source.height * f64::from(attachment[1])
                / f64::from(frame.extent[1]),
        );
        let v = scales
            .entry(d.asset.handle)
            .or_insert((d.asset.width, d.asset.height, 0., 0));
        v.2 = v.2.max(scale);
        v.3 += 1;
    }
    serde_json::json!(scales.iter().map(|(h, (w, hgt, scale, n))| serde_json::json!({
        "handle":format!("{h:x}"),"source":[w,hgt],"max_attachment_pixels_per_texel":scale,"draws":n
    })).collect::<Vec<_>>())
}

async fn run(output: &Path) -> Result<()> {
    std::fs::create_dir(output).map_err(|e| e.to_string())?;
    let control = std::env::args().any(|s| s == "--control");
    let mut csv = String::from(
        "adapter,width,height,round,quality,aa,mode,zoom,frame,prepare_ns,submit_ns,gpu_queue_span_ns,draws,gpu_draws,samples,target_bytes\n",
    );
    let mut details = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for adapter in gpu::instance()
        .enumerate_adapters(if std::env::var_os("GRAPHICS_SETTINGS_VULKAN").is_some() {
            wgpu::Backends::VULKAN
        } else {
            wgpu::Backends::DX12
        })
        .await
    {
        let info = adapter.get_info();
        if !matches!(
            info.device_type,
            wgpu::DeviceType::IntegratedGpu | wgpu::DeviceType::DiscreteGpu
        ) {
            continue;
        }
        if !seen.insert((info.vendor, info.device)) {
            continue;
        }
        let name = format!("{:04x}-{:04x}", info.vendor, info.device);
        let mut r = gpu::Renderer::from_adapter_with_features(
            adapter,
            wgpu::TextureFormat::Rgba8Unorm,
            4,
            if control {
                wgpu::Features::empty()
            } else {
                wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS
            },
        )
        .await?;
        let scope = r.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let timer = (!control).then(|| Timer::new(&r));
        let sizes = if control {
            vec![[1920, 1080], [3840, 2160]]
        } else if info.device_type == wgpu::DeviceType::IntegratedGpu {
            vec![[1920, 1080]]
        } else {
            vec![[3840, 2160]]
        };
        for physical in sizes {
            let mut g = world()?;
            let extent = g.window_viewport(physical, 1.)?;
            let mut worlds = [g.client().clone(), g.client().clone()];
            worlds[1].wheel_zoom(0.065_f64.ln() / 1.12_f64.ln());
            let quality = g.quality();
            let mut identity = 10_000;
            for w in &mut worlds {
                for _ in 0..110 {
                    identity += 1;
                    let p =
                        render::prepare_gameplay(w, &mut g.bank, extent, 1., identity, quality)?;
                    g.bank.recycle_frame(p);
                }
            }
            let texture = support::offscreen::target(&r, physical);
            let view = texture.create_view(&Default::default());
            let mut presenter = WorldPresenter::new(&r);
            let presets = if control {
                vec![slither_client::settings::GraphicsQuality::Auto]
            } else {
                vec![
                    slither_client::settings::GraphicsQuality::Auto,
                    slither_client::settings::GraphicsQuality::Low,
                    slither_client::settings::GraphicsQuality::Medium,
                    slither_client::settings::GraphicsQuality::High,
                ]
            };
            let aa_levels = if control {
                vec![slither_client::settings::AntiAliasing::Auto]
            } else {
                vec![
                    slither_client::settings::AntiAliasing::Auto,
                    slither_client::settings::AntiAliasing::Off,
                    slither_client::settings::AntiAliasing::X2,
                    slither_client::settings::AntiAliasing::X4,
                ]
            };
            for round in 0..if control { 1 } else { 5 } {
                let mut cells = Vec::new();
                for &preset in &presets {
                    for &aa in &aa_levels {
                        cells.push((preset, aa));
                    }
                }
                if round % 2 == 1 {
                    cells.reverse();
                }
                for (preset, aa) in cells {
                    let mut state =
                        slither_client::presentation::QualityState::new(preset.mode(), 0.)
                            .map_err(|e| e.to_string())?;
                    state.begin_connection();
                    let quality = if control { quality } else { state.snapshot() };
                    r.anti_aliasing = aa;
                    let preset_label = preset.label().to_ascii_lowercase();
                    let aa_label = match aa {
                        slither_client::settings::AntiAliasing::Auto => "auto",
                        slither_client::settings::AntiAliasing::Off => "off",
                        slither_client::settings::AntiAliasing::X2 => "2",
                        slither_client::settings::AntiAliasing::X4 => "4",
                    };
                    let modes = if round % 2 == 0 {
                        [Original, Native]
                    } else {
                        [Native, Original]
                    };
                    for mode in modes {
                        for (level, w) in worlds.iter_mut().enumerate() {
                            let zoom = if level == 0 { "normal" } else { "far" };
                            if level == 1 && (w.camera().user_zoom_factor() - 0.065).abs() > 1e-12 {
                                return Err("far zoom did not reach factor 0.065".into());
                            }
                            for index in 0..if control { 3 } else { 50 } {
                                identity += 1;
                                let start = Instant::now();
                                let p = render::prepare_gameplay(
                                    w,
                                    &mut g.bank,
                                    extent,
                                    1.,
                                    identity,
                                    quality,
                                )?;
                                let prepare = start.elapsed().as_nanos();
                                r.sync_resources(&p.resources)?;
                                wait(&r)?;
                                if let Some(timer) = &timer {
                                    timer.begin(&r);
                                }
                                let start = Instant::now();
                                let report = presenter
                                    .render(&mut r, &p.frame, None, &view, physical, mode)?;
                                let submit = start.elapsed().as_nanos();
                                let gpu_ns = if let Some(timer) = &timer {
                                    timer.end(&r)?
                                } else {
                                    wait(&r)?;
                                    0.
                                };
                                if !control && index >= 10 {
                                    writeln!(csv,"{name},{},{},{round},{preset_label},{aa_label},{mode:?},{zoom},{index},{prepare},{submit},{gpu_ns},{},{},{},{}",physical[0],physical[1],p.draw_count(),report.world_draws,report.plan.samples,report.target_bytes).unwrap();
                                }
                                if round == 0 && index == if control { 2 } else { 49 } {
                                    let file = format!(
                                        "{name}-{}x{}-{mode:?}-{zoom}-{preset_label}-{aa_label}.png",
                                        physical[0], physical[1]
                                    );
                                    if control {
                                        let mut encoded = flate2::write::ZlibEncoder::new(
                                            std::fs::File::create(
                                                output.join(file.replace(".png", ".draws.zlib")),
                                            )
                                            .map_err(|e| e.to_string())?,
                                            flate2::Compression::default(),
                                        );
                                        let source = format!(
                                            "{:?}\n{:?}",
                                            p.frame.canonical()?,
                                            p.resources
                                        );
                                        std::io::Write::write_all(&mut encoded, source.as_bytes())
                                            .map_err(|e| e.to_string())?;
                                        encoded.finish().map_err(|e| e.to_string())?;
                                    }
                                    let pixels = support::offscreen::pixels(&r, &texture)?;
                                    image::RgbaImage::from_raw(physical[0], physical[1], pixels)
                                        .ok_or("pixels")?
                                        .save(output.join(&file))
                                        .map_err(|e| e.to_string())?;
                                    details.push(serde_json::json!({"adapter":format!("{info:?}"),"physical":physical,"projection":extent,"mode":format!("{mode:?}"),"zoom":zoom,"zoom_factor":w.camera().user_zoom_factor(),"quality":preset_label,"aa":aa_label,"supported_samples":r.supported_samples,"file":file,"samples":report.plan.samples,"draws":p.draw_count(),"texel_scales":texel_scales(&p.frame,report.plan.attachment)}));
                                }
                                g.bank.recycle_frame(p);
                            }
                        }
                    }
                }
            }
        }
        if let Some(e) = scope.pop().await {
            return Err(e.to_string());
        }
    }
    if seen.is_empty() {
        return Err("No DX12 hardware adapter; do not substitute software rendering".into());
    }
    std::fs::write(output.join("frames.csv"), csv).map_err(|e| e.to_string())?;
    std::fs::write(
        output.join("details.json"),
        serde_json::to_vec_pretty(&details).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
fn main() {
    let output = std::env::args_os()
        .nth(1)
        .expect("fresh external output directory");
    if let Err(e) = pollster::block_on(run(Path::new(&output))) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
