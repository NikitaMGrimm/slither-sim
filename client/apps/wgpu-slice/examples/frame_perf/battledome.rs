//! Frozen production preparation + offscreen rendering, at physical 1080p/4K.
//! Each invocation is one A/B side; alternate retained binaries externally.
#[path = "../../tests/support/battledome_world.rs"]
mod synthetic;
use super::*;
use std::{fmt::Write as _, io::Write as _};

#[derive(Clone, Copy)]
struct Phase {
    stage: &'static str,
    id: u64,
    ns: u128,
}
thread_local! {
    static PHASES: std::cell::RefCell<(Instant, Vec<Phase>)> = std::cell::RefCell::new((Instant::now(), Vec::with_capacity(32)));
}
fn observe(stage: &'static str, id: u64) {
    PHASES.with_borrow_mut(|(start, rows)| {
        assert!(rows.len() < 32, "phase recorder overflow");
        rows.push(Phase {
            stage,
            id,
            ns: start.elapsed().as_nanos(),
        });
    });
}
struct Observing(Option<render::PreparationObserver>);
impl Drop for Observing {
    fn drop(&mut self) {
        render::set_preparation_observer(self.0);
    }
}

fn readback(r: &gpu::Renderer, buffer: &wgpu::Buffer) -> Result<Vec<u8>> {
    let (tx, rx) = mpsc::channel();
    buffer.slice(..).map_async(wgpu::MapMode::Read, move |v| {
        let _ = tx.send(v);
    });
    wait(r)?;
    rx.recv_timeout(Duration::from_secs(10))
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let bytes = buffer
        .slice(..)
        .get_mapped_range()
        .map_err(|e| e.to_string())?
        .to_vec();
    buffer.unmap();
    Ok(bytes)
}

fn gpu_ns(r: &gpu::Renderer) -> Result<f64> {
    let resolve = r.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("timestamp resolve"),
        size: 16,
        usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let read = r.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("timestamp readback"),
        size: 16,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = r.device.create_command_encoder(&Default::default());
    encoder.resolve_query_set(
        r.diagnostic_timestamps().ok_or("no timestamp queries")?,
        0..2,
        &resolve,
        0,
    );
    encoder.copy_buffer_to_buffer(&resolve, 0, &read, 0, 16);
    r.queue.submit([encoder.finish()]);
    let bytes = readback(r, &read)?;
    let begin = u64::from_le_bytes(bytes[..8].try_into().unwrap());
    let end = u64::from_le_bytes(bytes[8..].try_into().unwrap());
    Ok(end.wrapping_sub(begin) as f64 * f64::from(r.queue.get_timestamp_period()))
}

fn capture_png(r: &gpu::Renderer, texture: &wgpu::Texture, path: &Path) -> Result<()> {
    let size = texture.size();
    let stride = (size.width * 4).div_ceil(256) * 256;
    let buffer = r.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("battledome readback"),
        size: u64::from(stride) * u64::from(size.height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = r.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
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
                bytes_per_row: Some(stride),
                rows_per_image: Some(size.height),
            },
        },
        size,
    );
    r.queue.submit([encoder.finish()]);
    let bytes = readback(r, &buffer)?;
    let packed: Vec<_> = bytes
        .chunks_exact(stride as usize)
        .flat_map(|row| row[..size.width as usize * 4].iter().copied())
        .collect();
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut encoder = png::Encoder::new(file, size.width, size.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .map_err(|e| e.to_string())?
        .write_image_data(&packed)
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn run(output: &Path) -> Result<()> {
    std::fs::create_dir(output).map_err(|e| e.to_string())?;
    let instance = gpu::instance();
    let mut seen = std::collections::BTreeSet::new();
    for adapter in pollster::block_on(instance.enumerate_adapters(wgpu::Backends::DX12)) {
        let info = adapter.get_info();
        if info.device_type == wgpu::DeviceType::Cpu || !seen.insert((info.vendor, info.device)) {
            continue;
        }
        if !adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return Err(format!("no timestamps: {info:?}"));
        }
        let dir = output.join(format!("{:04x}-{:04x}", info.vendor, info.device));
        std::fs::create_dir(&dir).map_err(|e| e.to_string())?;
        let mut r = pollster::block_on(gpu::Renderer::from_adapter_for_diagnostics(
            adapter,
            wgpu::TextureFormat::Rgba8Unorm,
            4,
            true,
        ))?;
        std::fs::write(
            dir.join("adapter.txt"),
            format!("{info:?}\nsamples={}\n", r.sample_count),
        )
        .map_err(|e| e.to_string())?;
        let mut csv = String::from(
            "bots,width,height,level,samples,frame,scale,snakes,draws,vertices,draw_calls,prepare_ns,hud_ns,sync_ns,submit_ns,gpu_ns,prepare_bytes,hud_bytes,sync_bytes,submit_bytes,quad_area_px\n",
        );
        let mut phases = String::from("bots,width,level,samples,frame,stage,ns\n");
        let profile = std::env::var_os("FRAME_PERF_PHASES").is_some();
        let plan_only = std::env::var_os("FRAME_PERF_PLAN_ONLY").is_some();
        let capture_pixels = std::env::var_os("FRAME_PERF_NO_PNG").is_none();
        let mut failures = String::from("bots,width,level,error\n");
        for bots in [120, 500] {
            let game = synthetic::world(bots)?;
            let quality = game.quality();
            let hud = game.hud_view();
            for extent in [[1920, 1080], [3840, 2160]] {
                let texture = r.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("Battledome target"),
                    size: wgpu::Extent3d {
                        width: extent[0],
                        height: extent[1],
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                });
                let view = texture.create_view(&Default::default());
                'level: for (level, notches) in [("default", 0.), ("mid", -12.), ("out", -1000.)] {
                    r.sync_resources(&[])?;
                    let mut world = game.client().clone();
                    let mut bank = slither_wgpu_slice::bank::Bank::new()?;
                    world.wheel_zoom(notches);
                    for frame in 0..110 {
                        let result = render::prepare_gameplay(
                            &mut world,
                            &mut bank,
                            extent,
                            1.,
                            frame + 10000,
                            quality,
                        );
                        let p = match result {
                            Ok(p) => p,
                            Err(error) if error == slither_wgpu_slice::frame::DRAW_MEMORY_LIMIT => {
                                writeln!(failures, "{bots},{},{level},{error}", extent[0]).unwrap();
                                std::fs::write(dir.join("failures.csv"), &failures)
                                    .map_err(|e| e.to_string())?;
                                println!(
                                    "REJECTED bots={bots} extent={extent:?} zoom={level}: {error}"
                                );
                                continue 'level;
                            }
                            Err(error) => return Err(error),
                        };
                        if frame == 109 {
                            if level == "out"
                                && p.frame.snakes.len()
                                    != (usize::from(bots) + 1)
                                        .min(slither_client::state::MAX_SNAKES)
                            {
                                return Err("whole-map fixture did not include every snake".into());
                            }
                            println!(
                                "PLAN bots={bots} extent={extent:?} zoom={level} snakes={} body={} draws={}",
                                p.frame.snakes.len(),
                                p.frame
                                    .snakes
                                    .iter()
                                    .map(|s| s.body_commands)
                                    .sum::<usize>(),
                                p.draw_count()
                            );
                        }
                        bank.recycle_frame(p);
                    }
                    if plan_only {
                        continue;
                    }
                    // Compare 4x with 1x to isolate fill/MSAA sensitivity. CPU
                    // preparation and projection are identical in both cases.
                    for samples in [4, 1] {
                        for frame in 0..40_u64 {
                            let recording = profile.then(|| {
                                PHASES.with_borrow_mut(|(start, rows)| {
                                    *start = Instant::now();
                                    rows.clear();
                                });
                                Observing(render::set_preparation_observer(Some(observe)))
                            });
                            let scope = r.device.push_error_scope(wgpu::ErrorFilter::Validation);
                            let (p, prep, prep_bytes) = measure(|| {
                                render::prepare_gameplay(
                                    &mut world,
                                    &mut bank,
                                    extent,
                                    1.,
                                    20000 + frame,
                                    quality,
                                )
                            })?;
                            let (p, hud_ns, hud_bytes) =
                                measure(|| render::finish_hud(p, &mut bank, &hud))?;
                            if profile {
                                observe("build_end", 20000 + frame);
                            }
                            drop(recording);
                            let (_, sync_ns, sync_bytes) =
                                measure(|| r.sync_resources(&p.resources).map_err(Into::into))?;
                            let (_, submit_ns, submit_bytes) = measure(|| {
                                r.render_world_at(&p.frame, &view, extent, samples)
                                    .map_err(Into::into)
                            })?;
                            let calls = r.draw_call_count();
                            let gpu = gpu_ns(&r)?;
                            if let Some(error) = pollster::block_on(scope.pop()) {
                                return Err(format!("GPU validation: {error}"));
                            }
                            if frame >= 10 {
                                let area: f64 = p
                                    .frame
                                    .draws
                                    .iter()
                                    .map(|d| (d.destination.width * d.destination.height).abs())
                                    .sum();
                                writeln!(csv, "{bots},{},{},{level},{samples},{frame},{:.12},{},{},{},{calls},{prep},{hud_ns},{sync_ns},{submit_ns},{gpu:.0},{prep_bytes},{hud_bytes},{sync_bytes},{submit_bytes},{area:.0}",
                                    extent[0], extent[1], world.camera().zoom(), p.frame.snakes.len(), p.draw_count(), p.draw_count() * 6).unwrap();
                            }
                            if samples == 4 && frame == 39 {
                                let label = format!("{bots}-{}-{level}", extent[0]);
                                if capture_pixels {
                                    capture_png(&r, &texture, &dir.join(format!("{label}.png")))?;
                                }
                                if level == "default" {
                                    let file = std::fs::File::create(
                                        dir.join(format!("{label}.draws.gz")),
                                    )
                                    .map_err(|e| e.to_string())?;
                                    let mut zip = flate2::write::GzEncoder::new(
                                        file,
                                        flate2::Compression::fast(),
                                    );
                                    write!(zip, "{:?}", p.frame).map_err(|e| e.to_string())?;
                                    zip.finish().map_err(|e| e.to_string())?;
                                }
                            }
                            bank.recycle_frame(p);
                            if profile {
                                PHASES.with_borrow(|(_, rows)| {
                                    if frame >= 10 {
                                        for pair in rows.windows(2) {
                                            if pair[1].id == 20000 + frame
                                                && pair[0].id == pair[1].id
                                            {
                                                writeln!(
                                                    phases,
                                                    "{bots},{},{level},{samples},{frame},{},{}",
                                                    extent[0],
                                                    pair[1].stage,
                                                    pair[1].ns - pair[0].ns
                                                )
                                                .unwrap();
                                            }
                                        }
                                    }
                                });
                            }
                        }
                        println!(
                            "{info:?} bots={bots} extent={extent:?} zoom={level} samples={samples} PASS"
                        );
                    }
                }
            }
        }
        std::fs::write(dir.join("frames.csv"), &csv).map_err(|e| e.to_string())?;
        if profile {
            std::fs::write(dir.join("phases.csv"), &phases).map_err(|e| e.to_string())?;
        }
        std::fs::write(dir.join("failures.csv"), &failures).map_err(|e| e.to_string())?;
    }
    if seen.is_empty() {
        return Err("no hardware adapters".into());
    }
    Ok(())
}

/// A single local-host cross-check, deliberately excluded from frozen A/B data.
pub fn live(endpoint: &str, output: &Path) -> Result<()> {
    use slither_client::session_policy::Phase;
    use slither_wgpu_slice::{
        game::{EffectKind, LocalGame, loopback_endpoint},
        native::{Event, Transport},
    };
    let url = format!("ws://{endpoint}/slither");
    let port = loopback_endpoint(&url)?;
    if [8766, 18884, 18885].contains(&port) {
        return Err("reserved port".into());
    }
    std::fs::create_dir(output).map_err(|e| e.to_string())?;
    let instance = gpu::instance();
    let mut renderer = pollster::block_on(gpu::Renderer::new(
        &instance,
        None,
        wgpu::TextureFormat::Rgba8Unorm,
    ))?;
    let extent = [1920, 1080];
    let texture = renderer.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("loopback Battledome"),
        size: wgpu::Extent3d {
            width: extent[0],
            height: extent[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let mut game = LocalGame::new(0.)?;
    game.draw_diagnostics.strict_validation = true;
    let mut transport = Transport::with_wake(|| {});
    let start = Instant::now();
    game.play(&url, "Zoom probe", 0)?;
    let mut playing = None;
    let mut captured = false;
    let mut peak_received = 0;
    let mut peak_visible = 0;
    let mut pictures = 0;
    let result = (|| {
        while start.elapsed() < Duration::from_secs(60) {
            let now = start.elapsed().as_secs_f64() * 1000.;
            for event in transport.poll() {
                match event {
                    Event::Opened(g) => game.opened(g, now)?,
                    Event::Binary(g, bytes) => game.receive(g, &bytes, now)?,
                    Event::Closed(g) => {
                        game.closed(g);
                        return Err("local host closed".into());
                    }
                    Event::Failed(_, error) => return Err(error),
                }
            }
            if game.phase() == Phase::Playing {
                playing.get_or_insert_with(Instant::now);
                game.wheel_zoom(-1000.);
                peak_received = peak_received.max(game.client().snake_objects().count());
            }
            if matches!(game.phase(), Phase::Dead | Phase::Victory | Phase::Failed) {
                return Err(format!("live phase {:?}: {:?}", game.phase(), game.failure));
            }
            game.begin_redraw(now, extent, 1.)?;
            for _ in 0..64 {
                let Some(e) = game.effect() else {
                    break;
                };
                if matches!(e.kind, EffectKind::Connect(_)) {
                    game.construct(e.generation, e.id, now)?;
                }
                transport.execute(&e)?;
                game.acknowledge(e.generation, e.id, true)?;
            }
            if let Some(p) = game.take_frame() {
                renderer.sync_resources(&p.resources)?;
                renderer.render(&p.frame, &view)?;
                wait(&renderer)?;
                game.presented(true)?;
                pictures += 1;
                peak_visible = peak_visible.max(p.frame.snakes.len());
                if playing.is_some_and(|t| t.elapsed() > Duration::from_secs(3)) && !captured {
                    capture_png(&renderer, &texture, &output.join("whole-map.png"))?;
                    captured = true;
                }
                game.bank.recycle_frame(p);
            }
            if playing.is_some_and(|t| t.elapsed() > Duration::from_secs(5)) {
                break;
            }
            std::thread::sleep(Duration::from_millis(8));
        }
        if !captured {
            return Err("no settled live whole-map picture before deadline".into());
        }
        std::fs::write(output.join("summary.txt"), format!("received_peak={peak_received}\nvisible_peak={peak_visible}\npictures={pictures}\nzoom={}\nskipped={}\n", game.client().camera().zoom(), game.draw_diagnostics.skipped_frames)).map_err(|e| e.to_string())?;
        Ok(())
    })();
    transport.close();
    result
}
