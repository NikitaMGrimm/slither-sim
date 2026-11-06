//! Authored 25,000-food controls and CPU preparation/submission timings.
//! No window, transport, external art or network work. Hold the lane timing lock.
#[path = "../../../tests/support/protocol13.rs"]
mod authored;
#[path = "../tests/support/announcement_tape.rs"]
mod tape;
#[allow(dead_code)]
mod support {
    pub mod offscreen;
}
use slither_client::{
    presentation::{AnimatedClient, QualityState},
    settings::{GraphicsQuality, WorldResolution},
};
use slither_wgpu_slice::{Result, game::LocalGame, gpu, render, world_presenter::WorldPresenter};
use std::{io::Write, path::Path, time::Instant};

fn scene() -> Result<LocalGame> {
    let mut g = tape::negotiating();
    let mut setup = authored::setup();
    // The large authored arena admits distinct 0.065 and whole-map endpoints.
    setup[1..4].copy_from_slice(&authored::u24(32767));
    g.receive(
        g.generation(),
        &authored::bundle(&[setup, authored::snake(7)]),
        3.,
    )?;
    tape::pictures(&mut g);
    for block in 0..5_u16 {
        let mut packet = vec![b'F'];
        for i in block * 5000..(block + 1) * 5000 {
            packet.extend(&authored::food(b'f', 9, 1200 + i % 200, 1050 + i / 200, 1)[1..]);
        }
        g.receive(g.generation(), &packet, 21. + f64::from(block))?;
        tape::pictures(&mut g);
    }
    assert_eq!(g.client().foods().foods().len(), 25_000);
    Ok(g)
}

fn advance(world: &mut AnimatedClient, steps: usize) -> Result<()> {
    for step in 0..steps {
        world
            .advance(100. + step as f64 * 16., false)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
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

async fn run(output: &Path, adapter: &str, controls: bool) -> Result<()> {
    std::fs::create_dir(output).map_err(|e| e.to_string())?;
    let (backend, kind) = match adapter {
        "rtx" => (wgpu::Backends::VULKAN, wgpu::DeviceType::DiscreteGpu),
        "amd" => (wgpu::Backends::DX12, wgpu::DeviceType::IntegratedGpu),
        _ => return Err("adapter must be rtx or amd".into()),
    };
    let a = gpu::instance()
        .enumerate_adapters(backend)
        .await
        .into_iter()
        .find(|a| a.get_info().device_type == kind)
        .ok_or("requested hardware adapter unavailable")?;
    let info = a.get_info();
    let mut r =
        gpu::Renderer::from_adapter_with_max_samples(a, wgpu::TextureFormat::Rgba8Unorm, 4).await?;
    let scope = r.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let physical = [3840, 2160];
    let target = support::offscreen::target(&r, physical);
    let target_view = target.create_view(&Default::default());
    let mut g = scene()?;
    let extent = g.window_viewport(physical, 1.)?;
    let source = g.client().clone();
    let mut journal =
        std::fs::File::create(output.join("frames.csv")).map_err(|e| e.to_string())?;
    writeln!(journal, "order,preset,zoom,mode,state,frame,prepare_ns,sync_ns,submit_ns,wait_ns,draws,samples,zoom_factor,retained").map_err(|e| e.to_string())?;
    let mut identities = Vec::new();
    let mut identity = 10000;
    let presets = [
        GraphicsQuality::Auto,
        GraphicsQuality::Low,
        GraphicsQuality::Medium,
        GraphicsQuality::High,
    ];
    // Timing first traverses every preset untimed: the first preset in a
    // process otherwise prepares faster than any later visit. Measured passes
    // then go forward and reversed, because machine clock/core state drifts
    // within a run, so no preset keeps the first or last position.
    let passes: &[(&str, bool)] = if controls {
        &[("forward", false)]
    } else {
        &[("warmup", false), ("forward", false), ("reverse", true)]
    };
    let order = passes.iter().flat_map(|&(name, reverse)| {
        let mut p = presets;
        if reverse {
            p.reverse();
        }
        p.map(|preset| (name, preset))
    });
    for (order, preset) in order {
        let label = preset.label().to_ascii_lowercase();
        let mut quality = QualityState::new(preset.mode(), 0.).map_err(|e| e.to_string())?;
        quality.begin_connection();
        for (zoom, notches) in [
            ("stock", 0.),
            ("far", 0.065_f64.ln() / 1.12_f64.ln()),
            ("whole", -1000.),
        ] {
            for mode in [WorldResolution::Native, WorldResolution::Original] {
                let states: &[usize] = if controls { &[1, 12, 240] } else { &[240] };
                for &state in states {
                    let mut world = source.clone();
                    advance(&mut world, state)?;
                    world.wheel_zoom(notches);
                    // Settle camera/zoom independently of the frozen animation state.
                    for _ in 0..110 {
                        identity += 1;
                        let p = render::prepare_gameplay(
                            &mut world,
                            &mut g.bank,
                            extent,
                            1.,
                            identity,
                            quality.snapshot(),
                        )?;
                        g.bank.recycle_frame(p);
                    }
                    if zoom == "far" && (world.camera().user_zoom_factor() - 0.065).abs() > 1e-12 {
                        return Err("far endpoint is not 0.065".into());
                    }
                    if zoom == "whole" && world.camera().user_zoom_factor() >= 0.065 {
                        return Err("whole map is not distinct from far endpoint".into());
                    }
                    let mut presenter = WorldPresenter::new(&r);
                    for frame in 0..if controls { 3 } else { 60 } {
                        identity += 1;
                        let start = Instant::now();
                        let p = render::prepare_gameplay(
                            &mut world,
                            &mut g.bank,
                            extent,
                            1.,
                            identity,
                            quality.snapshot(),
                        )?;
                        let prepare = start.elapsed().as_nanos();
                        let start = Instant::now();
                        r.sync_resources(&p.resources)?;
                        let sync = start.elapsed().as_nanos();
                        wait(&r)?;
                        let start = Instant::now();
                        let report = presenter.render(
                            &mut r,
                            &p.frame,
                            p.overlay.as_ref(),
                            &target_view,
                            physical,
                            mode,
                        )?;
                        let submit = start.elapsed().as_nanos();
                        let start = Instant::now();
                        wait(&r)?;
                        let wait_ns = start.elapsed().as_nanos();
                        if !controls && order != "warmup" && frame >= 10 {
                            writeln!(journal, "{order},{label},{zoom},{mode:?},{state},{frame},{prepare},{sync},{submit},{wait_ns},{},{},{},{}", p.draw_count(), report.plan.samples, world.camera().user_zoom_factor(), p.frame.food_batch.is_some()).map_err(|e| e.to_string())?;
                        }
                        if controls && frame == 2 {
                            let file = format!("{label}-{zoom}-{mode:?}-{state}.rgba.zlib");
                            let pixels = support::offscreen::pixels(&r, &target)?;
                            let mut encoded = flate2::write::ZlibEncoder::new(
                                std::fs::File::create(output.join(&file))
                                    .map_err(|e| e.to_string())?,
                                flate2::Compression::default(),
                            );
                            encoded.write_all(&pixels).map_err(|e| e.to_string())?;
                            encoded.finish().map_err(|e| e.to_string())?;
                            identities.push(serde_json::json!({"preset":label,"zoom":zoom,"mode":format!("{mode:?}"),"state":state,"file":file,"zoom_factor":world.camera().user_zoom_factor(),"samples":report.plan.samples,"draws":p.draw_count(),"retained":p.frame.food_batch.is_some()}));
                        }
                        g.bank.recycle_frame(p);
                    }
                }
            }
        }
    }
    if let Some(e) = scope.pop().await {
        return Err(e.to_string());
    }
    std::fs::write(output.join("details.json"), serde_json::to_vec_pretty(&serde_json::json!({"adapter":{"name":info.name,"backend":format!("{:?}",info.backend),"driver":info.driver,"driver_info":info.driver_info},"physical":physical,"projection":extent,"controls":identities,"complete":true})).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 || !["control", "timing"].contains(&args[3].as_str()) {
        return Err("low_preset_perf NEW_DIRECTORY rtx|amd control|timing".into());
    }
    pollster::block_on(run(Path::new(&args[1]), &args[2], args[3] == "control"))
}
