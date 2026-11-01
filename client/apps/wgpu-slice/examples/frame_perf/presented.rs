//! Actual native projection/presentation boundary; no window or socket.
use super::*;
use slither_client::settings::WorldResolution::{Native, Original};
use slither_wgpu_slice::{world_presenter::WorldPresenter, world_resolution::WorldWindow};

fn pass_times(r: &gpu::Renderer) -> Result<Vec<(&'static str, f64)>> {
    let count = r.diagnostic_passes().len() as u32 * 2;
    if count == 0 {
        return Err("no diagnostic render passes".into());
    }
    let size = u64::from(count) * 8;
    let resolve = r.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("per-pass resolve"),
        size,
        usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let read = r.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("per-pass readback"),
        size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut e = r.device.create_command_encoder(&Default::default());
    e.resolve_query_set(
        r.diagnostic_timestamps()
            .ok_or("missing diagnostic queries")?,
        0..count,
        &resolve,
        0,
    );
    e.copy_buffer_to_buffer(&resolve, 0, &read, 0, size);
    r.queue.submit([e.finish()]);
    let bytes = readback(r, &read)?;
    r.diagnostic_passes()
        .iter()
        .map(|&(name, start)| {
            let at = start as usize * 8;
            let a = u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap());
            let b = u64::from_le_bytes(bytes[at + 8..at + 16].try_into().unwrap());
            Ok((
                name,
                b.checked_sub(a).ok_or("reversed GPU timestamp")? as f64
                    * f64::from(r.queue.get_timestamp_period()),
            ))
        })
        .collect()
}

pub fn run_created(output: &Path) -> Result<()> {
    let dpr = std::env::var("FRAME_PERF_DPR")
        .unwrap_or_else(|_| "1".into())
        .parse::<f64>()
        .map_err(|e| e.to_string())?;
    let profile = std::env::var_os("FRAME_PERF_PHASES").is_some();
    let plan_only = std::env::var_os("FRAME_PERF_PLAN_ONLY").is_some();
    let capture_pixels = std::env::var_os("FRAME_PERF_NO_PNG").is_none();
    let trace_root = std::env::var_os("FRAME_PERF_GPU_TRACE_DIR").map(std::path::PathBuf::from);
    // A validation capture needs one complete API-input picture per cell, not
    // thirty timing samples. Retain all creation/upload actions before it.
    let rendered_frames = if trace_root.is_some() { 1 } else { 40 };
    let fresh = std::env::var_os("FRAME_PERF_FRESH_CONTROL").is_some();
    let controls_only = fresh || std::env::var_os("FRAME_PERF_CONTROL_ONLY").is_some();
    let isolated_width = if fresh {
        Some(
            std::env::var("FRAME_PERF_WIDTH")
                .map_err(|_| "fresh control requires FRAME_PERF_WIDTH")?
                .parse::<u32>()
                .map_err(|e| e.to_string())?,
        )
    } else {
        None
    };
    let isolated_mode = if fresh {
        Some(
            std::env::var("FRAME_PERF_MODE")
                .map_err(|_| "fresh control requires FRAME_PERF_MODE")?,
        )
    } else {
        None
    };
    if isolated_width.is_some_and(|w| ![1920, 3840].contains(&w))
        || isolated_mode
            .as_ref()
            .is_some_and(|m| m != "Original" && m != "Native")
    {
        return Err("invalid isolated control mode/width".into());
    }
    let history = if fresh { "fresh" } else { "sequence" };
    let instance = gpu::instance();
    let mut seen = std::collections::BTreeSet::new();
    for adapter in pollster::block_on(instance.enumerate_adapters(wgpu::Backends::DX12)) {
        let info = adapter.get_info();
        if info.device_type == wgpu::DeviceType::Cpu || !seen.insert((info.vendor, info.device)) {
            continue;
        }
        let features =
            wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
        if !adapter.features().contains(features) {
            return Err(format!(
                "adapter lacks per-pass/encoder timestamps: {info:?}"
            ));
        }
        let dir = output.join(format!("{:04x}-{:04x}", info.vendor, info.device));
        std::fs::create_dir(&dir).map_err(|e| e.to_string())?;
        std::fs::write(dir.join("adapter.txt"), format!("{info:?}\n"))
            .map_err(|e| e.to_string())?;
        let mut csv = String::from(
            "bots,width,height,backing_width,backing_height,attachment_width,attachment_height,dpr,requested_mode,effective_mode,requested_samples,samples,overlay_samples,history,level,frame,scale,snakes,draws,vertices,draw_calls,prepare_ns,hud_ns,sync_ns,submit_ns,gpu_ns,world_gpu_ns,blit_gpu_ns,overlay_gpu_ns,prepare_bytes,hud_bytes,sync_bytes,submit_bytes,quad_area_px,capture_kind\n",
        );
        let mut phases = String::from(
            "bots,width,height,dpr,mode,requested_samples,samples,level,frame,stage,ns\n",
        );
        for requested_samples in [4, 1] {
            let make_renderer = || {
                pollster::block_on(gpu::Renderer::from_adapter_for_diagnostics(
                    adapter.clone(),
                    wgpu::TextureFormat::Rgba8Unorm,
                    requested_samples,
                    true,
                ))
            };
            let mut r = make_renderer()?;
            let mut presenter = WorldPresenter::new(&r);
            for requested in [Original, Native] {
                if isolated_mode
                    .as_ref()
                    .is_some_and(|m| m != &format!("{requested:?}"))
                {
                    continue;
                }
                for bots in [120, 500] {
                    if controls_only && bots != 500 {
                        continue;
                    }
                    for physical in [[1920, 1080], [3840, 2160]] {
                        if isolated_width.is_some_and(|w| w != physical[0]) {
                            continue;
                        }
                        let mut game = synthetic::world(bots)?;
                        let window = WorldWindow {
                            physical,
                            dpr,
                            requested,
                        };
                        let projection = window.viewport(&mut game)?;
                        let quality = game.quality();
                        let hud = game.hud_view();
                        let texture = r.device.create_texture(&wgpu::TextureDescriptor {
                            label: Some("Battledome physical target"),
                            size: wgpu::Extent3d {
                                width: physical[0],
                                height: physical[1],
                                depth_or_array_layers: 1,
                            },
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: wgpu::TextureDimension::D2,
                            format: wgpu::TextureFormat::Rgba8Unorm,
                            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                                | wgpu::TextureUsages::COPY_SRC,
                            view_formats: &[],
                        });
                        let view = texture.create_view(&Default::default());
                        for (level, notches) in [("default", 0.), ("mid", -12.), ("out", -1000.)] {
                            if controls_only && level != "default" {
                                continue;
                            }
                            r.sync_resources(&[])?;
                            let mut world = game.client().clone();
                            let mut bank = slither_wgpu_slice::bank::Bank::new()?;
                            bank.overlay_viewport = game.bank.overlay_viewport;
                            world.wheel_zoom(notches);
                            for frame in 0..110 {
                                let p = render::prepare_gameplay(
                                    &mut world,
                                    &mut bank,
                                    projection,
                                    1.,
                                    10000 + frame,
                                    quality,
                                )?;
                                if frame == 109 {
                                    if level == "out"
                                        && p.frame.snakes.len()
                                            != (usize::from(bots) + 1)
                                                .min(slither_client::state::MAX_SNAKES)
                                    {
                                        return Err("whole-map fixture omitted snakes".into());
                                    }
                                    println!(
                                        "PLAN bots={bots} extent={physical:?} zoom={level} mode={requested:?} requested_samples={requested_samples} projection={projection:?} dpr={dpr} draws={}",
                                        p.draw_count()
                                    );
                                }
                                bank.recycle_frame(p);
                            }
                            if plan_only {
                                continue;
                            }
                            for frame in 0..rendered_frames {
                                let label = format!(
                                    "frame-perf:{bots}:{}:{dpr}:{requested:?}:{requested_samples}:{level}:{frame}",
                                    physical[0]
                                );
                                if trace_root.is_some() {
                                    r.diagnostic_trace_marker(&format!("{label}:begin"));
                                }
                                r.begin_diagnostic_frame();
                                let scope =
                                    r.device.push_error_scope(wgpu::ErrorFilter::Validation);
                                let observing = profile.then(|| {
                                    PHASES.with_borrow_mut(|(start, rows)| {
                                        *start = Instant::now();
                                        rows.clear();
                                    });
                                    Observing(render::set_preparation_observer(Some(observe)))
                                });
                                let (p, prep, prep_bytes) = measure(|| {
                                    render::prepare_gameplay(
                                        &mut world,
                                        &mut bank,
                                        projection,
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
                                drop(observing);
                                let (_, sync_ns, sync_bytes) =
                                    measure(|| r.sync_resources(&p.resources).map_err(Into::into))?;
                                let (report, submit_ns, submit_bytes) = measure(|| {
                                    presenter
                                        .render(
                                            &mut r,
                                            &p.frame,
                                            p.overlay.as_ref(),
                                            &view,
                                            physical,
                                            requested,
                                        )
                                        .map_err(Into::into)
                                })?;
                                let times = pass_times(&r)?;
                                if let Some(error) = pollster::block_on(scope.pop()) {
                                    return Err(format!("GPU validation: {error}"));
                                }
                                if trace_root.is_some() {
                                    r.diagnostic_trace_marker(&format!("{label}:end"));
                                    wait(&r)?;
                                }
                                if frame >= 10 || trace_root.is_some() {
                                    let time = |name| {
                                        times.iter().find(|x| x.0 == name).map_or(0., |x| x.1)
                                    };
                                    let area: f64 = p
                                        .frame
                                        .draws
                                        .iter()
                                        .map(|d| (d.destination.width * d.destination.height).abs())
                                        .sum();
                                    let plan = report.plan;
                                    writeln!(csv,"{bots},{},{},{},{},{},{},{dpr},{requested:?},{:?},{requested_samples},{},1,{history},{level},{frame},{:.12},{},{},{},{},{prep},{hud_ns},{sync_ns},{submit_ns},{:.0},{:.0},{:.0},{:.0},{prep_bytes},{hud_bytes},{sync_bytes},{submit_bytes},{area:.0},{}",
                                        physical[0],physical[1],projection[0],projection[1],plan.attachment[0],plan.attachment[1],plan.effective,plan.samples,world.camera().zoom(),p.frame.snakes.len(),p.draw_count(),p.draw_count()*6,
                                        report.world_draws+report.overlay_draws+usize::from(plan.blit),times.iter().map(|x|x.1).sum::<f64>(),time("world"),time("blit"),time("overlay"),if trace_root.is_some(){"input-trace"}else if profile{"profile"}else{"timing"}).unwrap();
                                    if profile {
                                        PHASES.with_borrow(|(_,rows)| { for pair in rows.windows(2) {
                                            if pair[0].id==20000+frame && pair[1].id==pair[0].id {
                                                writeln!(phases,"{bots},{},{},{dpr},{requested:?},{requested_samples},{},{level},{frame},{},{}",physical[0],physical[1],plan.samples,pair[1].stage,pair[1].ns-pair[0].ns).unwrap();
                                            }
                                        }});
                                    }
                                }
                                if frame + 1 == rendered_frames && requested_samples == 4 {
                                    let name =
                                        format!("{bots}-{}-{level}-{requested:?}", physical[0]);
                                    if capture_pixels {
                                        capture_png(
                                            &r,
                                            &texture,
                                            &dir.join(format!("{name}.png")),
                                        )?;
                                    }
                                    if level == "default" {
                                        let file = std::fs::File::create(
                                            dir.join(format!("{name}.draws.gz")),
                                        )
                                        .map_err(|e| e.to_string())?;
                                        let mut zip = flate2::write::GzEncoder::new(
                                            file,
                                            flate2::Compression::fast(),
                                        );
                                        write!(zip, "{:?}\n{:?}", p.frame, p.overlay)
                                            .map_err(|e| e.to_string())?;
                                        zip.finish().map_err(|e| e.to_string())?;
                                    }
                                }
                                bank.recycle_frame(p);
                                if let Some(root) = &trace_root {
                                    trace_payloads::archive(root)?;
                                }
                            }
                        }
                    }
                }
            }
        }
        std::fs::write(dir.join("frames.csv"), csv).map_err(|e| e.to_string())?;
        std::fs::write(dir.join("failures.csv"), "bots,width,level,error\n")
            .map_err(|e| e.to_string())?;
        if profile {
            std::fs::write(dir.join("phases.csv"), phases).map_err(|e| e.to_string())?;
        }
    }
    if seen.is_empty() {
        return Err("no hardware adapter exposed".into());
    }
    Ok(())
}
