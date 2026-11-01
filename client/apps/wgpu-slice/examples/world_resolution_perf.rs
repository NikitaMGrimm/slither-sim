//! Finite iGPU-only headless workload. All packets and artwork are authored here/in the crate.
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
    let mut g = tape::playing();
    // Exercise standard shaded skin sources and authored procedural cosmetics,
    // without supplying any external image or touching an art-pack directory.
    g.bank.cosmetics = Some(slither_wgpu_slice::cosmetics::Cosmetics::from_images(
        Default::default(),
        None,
    )?);
    let mut packets = Vec::new();
    for i in 0..8_u16 {
        let mut p = tape::snake((i % 7) as u8);
        p[1..3].copy_from_slice(&(100 + i).to_be_bytes());
        let (x, y) = (850 + (i % 4) * 120, 850 + (i / 4) * 220);
        p[16..19].copy_from_slice(&tape::u24(u32::from(x) * 5));
        p[19..22].copy_from_slice(&tape::u24(u32::from(y) * 5));
        p[25..28].copy_from_slice(&tape::u24(u32::from(x) * 5 - 720));
        p[28..31].copy_from_slice(&tape::u24(u32::from(y) * 5));
        packets.push(p);
    }
    for i in 0..324_u16 {
        let (x, y) = (100 + i % 18 * 114, 100 + i / 18 * 114);
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
    let mut csv = String::from(
        "samples,width,height,round,mode,zoom,frame,prepare_ns,sync_ns,submit_ns,gpu_queue_span_ns,completion_ns,draws,pixel_bytes,target_bytes\n",
    );
    let mut details = Vec::new();
    // Measure the production device sample ceiling; Native may choose one.
    let samples = 4;
    let adapter = gpu::instance()
        .enumerate_adapters(wgpu::Backends::DX12)
        .await
        .into_iter()
        .find(|a| a.get_info().device_type == wgpu::DeviceType::IntegratedGpu)
        .ok_or("No DX12 integrated GPU; do not substitute a discrete/CPU adapter")?;
    println!("iGPU={:?}, maximum samples={samples}", adapter.get_info());
    let features =
        wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
    if !adapter.features().contains(features) {
        return Err("iGPU lacks encoder timestamps".into());
    }
    let mut r = gpu::Renderer::from_adapter_with_features(
        adapter,
        wgpu::TextureFormat::Rgba8Unorm,
        samples,
        features,
    )
    .await?;
    assert_eq!(r.sample_count, samples);
    let scope = r.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let timer = Timer::new(&r);
    for physical in [[1920, 1080], [2560, 1440], [3840, 2160]] {
        let mut g = world()?;
        let extent = g.window_viewport(physical, 1.)?;
        let mut worlds = [g.client().clone(), g.client().clone()];
        worlds[1].wheel_zoom(1000.);
        let quality = g.quality();
        let mut identity = 10_000;
        for w in &mut worlds {
            for _ in 0..110 {
                identity += 1;
                let p = render::prepare_gameplay(w, &mut g.bank, extent, 1., identity, quality)?;
                g.bank.recycle_frame(p);
            }
        }
        let texture = support::offscreen::target(&r, physical);
        let view = texture.create_view(&Default::default());
        let mut presenter = WorldPresenter::new(&r);
        for round in 0..5 {
            let modes = if round % 2 == 0 {
                [Original, Native]
            } else {
                [Native, Original]
            };
            for mode in modes {
                for (level, w) in worlds.iter_mut().enumerate() {
                    let zoom = if level == 0 { "stock" } else { "in" };
                    for index in 0..60 {
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
                        let start = Instant::now();
                        r.sync_resources(&p.resources)?;
                        let sync = start.elapsed().as_nanos();
                        wait(&r)?; // No backlog from preparation/upload or the prior picture.
                        timer.begin(&r);
                        let start = Instant::now();
                        let report =
                            presenter.render(&mut r, &p.frame, None, &view, physical, mode)?;
                        let submit = start.elapsed().as_nanos();
                        let gpu_ns = timer.end(&r)?;
                        let completion = start.elapsed().as_nanos();
                        assert_eq!(report.plan.effective, mode);
                        if index >= 20 {
                            writeln!(csv, "{samples},{},{},{round},{mode:?},{zoom},{index},{prepare},{sync},{submit},{gpu_ns},{completion},{},{},{}",
                                physical[0],physical[1],p.draw_count(),p.pixel_bytes(),report.target_bytes).unwrap();
                        }
                        if round == 0 && index == 59 {
                            let pixels = support::offscreen::pixels(&r, &texture)?;
                            let image =
                                image::RgbaImage::from_raw(physical[0], physical[1], pixels)
                                    .ok_or("pixels")?;
                            let file = format!(
                                "{mode:?}-{}x{}-max-msaa{samples}-world-msaa{}-{zoom}.png",
                                physical[0], physical[1], report.plan.samples
                            );
                            image.save(output.join(&file)).map_err(|e| e.to_string())?;
                            details.push(serde_json::json!({"adapter":format!("{:?}",r.adapter_info),"maximum_samples":samples,
                                "world_samples":report.plan.samples,"physical":physical,"projection":extent,"mode":format!("{mode:?}"),"zoom":zoom,
                                "file":file,"draws":p.draw_count(),"gpu_draw_calls":report.world_draws,"pixel_bytes":p.pixel_bytes(),
                                "target_bytes":report.target_bytes,"transition_bytes":report.transient_target_bytes,
                                "presentation_bytes":u64::from(physical[0])*u64::from(physical[1])*4,
                                "texel_scales":texel_scales(&p.frame,report.plan.attachment)}));
                        }
                        g.bank.recycle_frame(p);
                    }
                }
            }
        }
        println!(
            "PASS {physical:?}, maximum samples={samples}; 5 alternating rounds, 200 measured frames per mode/zoom"
        );
    }
    if let Some(e) = scope.pop().await {
        return Err(e.to_string());
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
        .expect("fresh external output directory or --validate-fixture");
    if output == "--validate-fixture" {
        let g = world().expect("settled synthetic fixture");
        println!(
            "PASS settled fixture: {} food, all fade=1/scale=1, phase={:?}; no GPU/window/socket",
            g.client().foods().foods().len(),
            g.phase()
        );
        return;
    }
    if let Err(e) = pollster::block_on(run(Path::new(&output))) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
