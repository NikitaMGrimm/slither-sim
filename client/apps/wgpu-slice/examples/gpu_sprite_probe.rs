//! Finite offline sprite diagnostic; authored synthetic or owner-local binary replay.
#[allow(dead_code)]
#[path = "support/offscreen.rs"]
mod offscreen;
#[path = "frame_perf/probe_synthetic.rs"]
mod synthetic_world;
#[path = "../tests/support/announcement_tape.rs"]
mod tape;
use slither_client::settings::WorldResolution;
use slither_wgpu_slice::{
    Result, endpoint_login,
    game::{EffectKind, LocalGame},
    gpu, render,
};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    fmt::Write as _,
    hash::{Hash, Hasher},
    time::{Duration, Instant},
};
thread_local! {static DIGEST:RefCell<std::collections::hash_map::DefaultHasher>=RefCell::new(std::collections::hash_map::DefaultHasher::new());}
fn ns(t: Instant) -> u64 {
    t.elapsed().as_nanos() as u64
}
fn effects(g: &mut LocalGame, now: f64) -> Result<()> {
    for _ in 0..256 {
        let Some(e) = g.effect() else { return Ok(()) };
        if std::env::var_os("GPU_SPRITE_PARITY").is_some()
            && let EffectKind::Send(bytes) = &e.kind
        {
            DIGEST.with_borrow_mut(|h| bytes.hash(h));
        }
        let connect = matches!(e.kind, EffectKind::Connect(_));
        if connect {
            g.construct(e.generation, e.id, now)?;
        }
        g.acknowledge(e.generation, e.id, true)?;
        if connect {
            g.opened(e.generation, now)?;
        }
    }
    Err("effect budget".into())
}
struct Gpu {
    presenter: slither_wgpu_slice::world_presenter::WorldPresenter,
    r: gpu::Renderer,
    _target: wgpu::Texture,
    view: wgpu::TextureView,
    physical: [u32; 2],
    policy: WorldResolution,
    selected_samples: u32,
    attachment: [u32; 2],
}
impl Gpu {
    fn new(physical: [u32; 2], samples: u32, policy: WorldResolution) -> Result<Self> {
        let i = gpu::instance();
        let a = pollster::block_on(i.enumerate_adapters(wgpu::Backends::DX12))
            .into_iter()
            .find(|a| a.get_info().vendor == 0x1002)
            .ok_or("AMD adapter missing")?;
        let r = pollster::block_on(gpu::Renderer::from_adapter_for_diagnostics(
            a,
            wgpu::TextureFormat::Rgba8Unorm,
            samples,
            true,
        ))?;
        println!("adapter={:?},samples={}", r.adapter_info, r.sample_count);
        let target = r.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("capture profile"),
            size: wgpu::Extent3d {
                width: physical[0],
                height: physical[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&Default::default());
        let presenter = slither_wgpu_slice::world_presenter::WorldPresenter::new(&r);
        Ok(Self {
            presenter,
            r,
            _target: target,
            view,
            physical,
            policy,
            selected_samples: 0,
            attachment: [0, 0],
        })
    }
    fn draw(&mut self, p: &render::PreparedFrame) -> Result<(u64, u64, u64, f64, usize)> {
        let t = Instant::now();
        self.r.sync_resources(&p.resources).map_err(String::from)?;
        let sync = ns(t);
        let t = Instant::now();
        self.r.begin_diagnostic_frame();
        let report = self
            .presenter
            .render(
                &mut self.r,
                &p.frame,
                p.overlay.as_ref(),
                &self.view,
                self.physical,
                self.policy,
            )
            .map_err(String::from)?;
        self.selected_samples = report.plan.samples;
        self.attachment = report.plan.attachment;
        let submit = ns(t);
        let gpu_ns = self.timestamp()?;
        let t = Instant::now();
        self.r
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(Duration::from_secs(10)),
            })
            .map_err(|e| e.to_string())?;
        Ok((
            sync,
            submit,
            ns(t),
            gpu_ns,
            report.world_draws + report.overlay_draws,
        ))
    }
    fn timestamp(&self) -> Result<f64> {
        let b = self.r.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 256,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let read = self.r.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut e = self.r.device.create_command_encoder(&Default::default());
        e.resolve_query_set(
            self.r.diagnostic_timestamps().unwrap(),
            0..(self.r.diagnostic_passes().len() as u32 * 2),
            &b,
            0,
        );
        e.copy_buffer_to_buffer(&b, 0, &read, 0, 256);
        self.r.queue.submit([e.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        read.slice(..).map_async(wgpu::MapMode::Read, move |x| {
            let _ = tx.send(x);
        });
        self.r
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(Duration::from_secs(10)),
            })
            .map_err(|e| e.to_string())?;
        rx.recv().unwrap().map_err(|e| e.to_string())?;
        let d = read
            .slice(..)
            .get_mapped_range()
            .map_err(|e| e.to_string())?;
        let mut total = 0;
        for i in 0..self.r.diagnostic_passes().len() {
            let a = u64::from_le_bytes(d[i * 16..i * 16 + 8].try_into().unwrap());
            let b = u64::from_le_bytes(d[i * 16 + 8..i * 16 + 16].try_into().unwrap());
            let span = b.saturating_sub(a);
            if std::env::var_os("GPU_SPRITE_PASSES").is_some() {
                println!(
                    "pass {}={:.0}",
                    self.r.diagnostic_passes()[i].0,
                    span as f64 * self.r.queue.get_timestamp_period() as f64
                );
            }
            total += span;
        }
        Ok(total as f64 * self.r.queue.get_timestamp_period() as f64)
    }
}

// Continuous triangle/square intersection area in NDC; not fragment invocations.
fn area(vertices: &[[f32; 5]; 6]) -> f64 {
    let [a, b, c, _, _, d] = *vertices;
    if a[0] == d[0] && b[0] == c[0] && a[1] == b[1] && c[1] == d[1] {
        let x = (f64::from(a[0].max(b[0])).min(1.) - f64::from(a[0].min(b[0])).max(-1.)).max(0.);
        let y = (f64::from(a[1].max(d[1])).min(1.) - f64::from(a[1].min(d[1])).max(-1.)).max(0.);
        return x * y / 4.;
    }
    let mut sum = 0.;
    for tri in vertices.as_chunks::<3>().0 {
        let mut p: Vec<[f64; 2]> = tri.iter().map(|v| [v[0] as f64, v[1] as f64]).collect();
        for (axis, sign) in [(0, 1.), (0, -1.), (1, 1.), (1, -1.)] {
            let old = std::mem::take(&mut p);
            if old.is_empty() {
                break;
            }
            let mut a = *old.last().unwrap();
            for b in old {
                let da = 1. - sign * a[axis];
                let db = 1. - sign * b[axis];
                if (da >= 0.) != (db >= 0.) {
                    let t = da / (da - db);
                    p.push([a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])]);
                }
                if db >= 0. {
                    p.push(b);
                }
                a = b;
            }
        }
        if p.len() >= 3 {
            sum += p
                .iter()
                .zip(p.iter().cycle().skip(1))
                .take(p.len())
                .map(|(a, b)| a[0] * b[1] - b[0] * a[1])
                .sum::<f64>()
                .abs()
                / 8.;
        }
    }
    sum
}
fn main() -> Result<()> {
    let a: Vec<_> = std::env::args().collect();
    if a.len() != 12 {
        return Err("gpu_sprite_probe replay|synthetic INPUT OUTPUT ZOOM WIDTH HEIGHT original|native SAMPLES all|no-glow|half-glow WARM COUNT".into());
    }
    let synth = a[1] == "synthetic";
    if !synth && a[1] != "replay" {
        return Err("offline only".into());
    }
    let physical = [a[5].parse().unwrap(), a[6].parse().unwrap()];
    let policy = if a[7] == "native" {
        WorldResolution::Native
    } else {
        WorldResolution::Original
    };
    let samples = a[8].parse().unwrap();
    let zoom: f64 = a[4].parse().unwrap();
    let warm: usize = a[10].parse().unwrap();
    let count: usize = a[11].parse().unwrap();
    let mut records = Vec::new();
    if !synth {
        let b = std::fs::read(&a[2]).map_err(|e| e.to_string())?;
        let mut i = 0;
        while i < b.len() {
            let t = u64::from_be_bytes(b[i..i + 8].try_into().unwrap()) as f64 / 1000.;
            let n = u32::from_be_bytes(b[i + 8..i + 12].try_into().unwrap()) as usize;
            i += 12;
            records.push((t, b[i..i + n].to_vec()));
            i += n;
        }
    }
    let mut g = if synth {
        synthetic_world::world(true)?
    } else {
        LocalGame::new(0.)?
    };
    let backing = g.window_viewport(physical, 1.)?;
    if !synth {
        endpoint_login::play(&mut g, "ws://127.0.0.1:9/slither", "", 0)?;
        effects(&mut g, 0.)?;
    }
    let mut classes = BTreeMap::new();
    for color in 0..10 {
        let f = g.bank.food(color, [0., 0.]);
        for s in f.bodies {
            classes.insert(s.asset().handle, 0usize);
        }
        for s in f.outlines {
            classes.insert(s.asset().handle, 1);
        }
        for s in f.glows {
            classes.insert(s.asset().handle, 2);
        }
    }
    let mut gpu = Gpu::new(physical, samples, policy)?;
    let mut cursor = 0;
    let mut zoomed = false;
    let mut out = String::from(
        "frame,repeat,model_ms,draws,calls,sync_ns,submit_ns,gpu_ns,body_quads,outline_quads,glow_quads,other_quads,body_screen_area,outline_screen_area,glow_screen_area,other_screen_area,attachment_pixels,samples,received_foods,received_snakes,picture\n",
    );
    let full = std::env::var_os("GPU_SPRITE_FULL").is_some();
    for frame in 0..if synth { 181 } else { 366 } {
        let model = frame as f64 * 1000. / 60. + if synth { 1000. } else { 0. };
        if !zoomed && g.phase() == slither_client::session_policy::Phase::Playing {
            g.wheel_zoom(zoom);
            zoomed = true;
        }
        while cursor < records.len() && records[cursor].0 <= model {
            let (t, b) = &records[cursor];
            g.receive(g.generation(), b, *t)?;
            effects(&mut g, *t)?;
            cursor += 1;
        }
        g.begin_redraw(model, backing, 1.)?;
        effects(&mut g, model)?;
        while let Some(mut p) = g.take_frame() {
            if std::env::var_os("GPU_SPRITE_PARITY").is_some() {
                DIGEST.with_borrow_mut(|h| {
                    format!("{:?}{:?}", p.frame, p.overlay).hash(h);
                    for r in &p.resources {
                        format!("{:?}", r.asset).hash(h);
                        r.image.width.hash(h);
                        r.image.height.hash(h);
                        r.image.pixels.hash(h);
                    }
                });
            }
            let selected = if synth {
                [60, 120, 180].contains(&frame)
            } else {
                [120, 240, 365].contains(&frame)
            };
            if full || selected {
                let resident = p
                    .resources
                    .iter()
                    .map(|r| (r.asset.into(), r.asset))
                    .collect();
                let mut areas = [0.; 4];
                let mut counts = [0usize; 4];
                for q in p.frame.quad_iter(&resident)? {
                    let q = q?;
                    let c = classes.get(&q.key.handle).copied().unwrap_or(3);
                    areas[c] += area(&q.vertices);
                    counts[c] += 1;
                }
                let mut seen = 0;
                p.frame.draws.retain(|d| {
                    if classes.get(&d.asset.handle) != Some(&2) {
                        true
                    } else {
                        seen += 1;
                        match a[9].as_str() {
                            "no-glow" => false,
                            "half-glow" => seen % 2 == 0,
                            _ => true,
                        }
                    }
                });
                for j in 0..warm + count {
                    let (s, u, _, n, c) = gpu.draw(&p)?;
                    if j >= warm {
                        writeln!(
                            out,
                            "{frame},{},{model},{},{c},{s},{u},{n},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                            j - warm,
                            p.draw_count(),
                            counts[0],
                            counts[1],
                            counts[2],
                            counts[3],
                            areas[0],
                            areas[1],
                            areas[2],
                            areas[3],
                            gpu.attachment[0]*gpu.attachment[1],
                            gpu.selected_samples,
                            g.client().received().foods.len(),
                            g.client().received().snakes.len(),
                            p.frame.identity
                        )
                        .unwrap();
                    }
                }
                if selected && std::env::var_os("GPU_SPRITE_READBACK").is_some() {
                    let pixels = offscreen::pixels(&gpu.r, &gpu._target)?;
                    let path = std::path::Path::new(&a[3]).with_extension(format!("{frame}.rgba"));
                    std::fs::write(path, pixels).map_err(|e| e.to_string())?;
                }
                if selected {
                    println!("snapshot={frame},quads={counts:?},screen_area={areas:?}");
                }
            }
            g.presented(true)?;
            g.bank.recycle_frame(p);
            effects(&mut g, model)?;
        }
    }
    assert_eq!(cursor, records.len());
    println!(
        "complete=true,messages={},digest={:016x},skips={}",
        cursor,
        DIGEST.with_borrow(|h| h.finish()),
        g.draw_diagnostics.skipped_frames
    );
    assert_eq!(g.draw_diagnostics.skipped_frames, 0);
    std::fs::write(&a[3], out).map_err(|e| e.to_string())?;
    Ok(())
}
