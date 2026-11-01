//! Authored synthetic pellet matrix. No window, socket, external source or art.
#[path = "../tests/support/announcement_tape.rs"]
mod tape;
#[allow(dead_code)]
mod support {
    pub mod offscreen;
    pub mod pellet_timing;
}
use slither_client::{presentation::QualityMode, settings::WorldResolution};
use slither_wgpu_slice::{Result, game::LocalGame, gpu, render, world_presenter::WorldPresenter};
use std::path::Path;

fn scene(out: bool, dpr: f64) -> Result<(LocalGame, [u32; 2])> {
    let physical = [1920., 1080.].map(|n| (n * dpr) as u32);
    let mut g = tape::negotiating();
    g.receive(
        g.generation(),
        &tape::bundle(&[tape::setup(), tape::snake(0)]),
        3.,
    )?;
    tape::pictures(&mut g);
    let mut packets = Vec::new();
    for i in 0..80_u16 {
        let (x, y) = (700 + (i % 10) * 90, 700 + (i / 10) * 90);
        packets.push(vec![
            b'F',
            (x / 100) as u8,
            (y / 100) as u8,
            (i % 10) as u8,
            ((x % 100) * 255 / 100) as u8,
            ((y % 100) * 255 / 100) as u8,
            (6 + (i % 8) * 5) as u8,
        ]);
    }
    g.receive(g.generation(), &tape::bundle(&packets), 21.)?;
    tape::pictures(&mut g);
    g.window_viewport(physical, dpr)?;
    if out {
        assert!(g.wheel_zoom(-1000.));
    }
    for step in 1..=240 {
        let now = 21. + f64::from(step) * 16.;
        let extent = g.window_viewport(physical, dpr)?;
        g.begin_redraw(now, extent, 1.)?;
        let packets = tape::effects(&mut g);
        tape::pictures(&mut g);
        if packets.iter().any(|p| p.as_slice() == [251]) {
            g.receive(g.generation(), b"p", now)?;
            tape::pictures(&mut g);
        }
    }
    assert!(
        g.client()
            .foods()
            .foods()
            .iter()
            .all(|f| f.snapshot().scale == 1.)
    );
    Ok((g, physical))
}

async fn run(output: &Path) -> Result<()> {
    std::fs::create_dir(output).map_err(|e| e.to_string())?;
    let adapter = gpu::instance()
        .enumerate_adapters(wgpu::Backends::DX12)
        .await
        .into_iter()
        .find(|a| a.get_info().device_type == wgpu::DeviceType::IntegratedGpu)
        .ok_or("No DX12 integrated GPU")?;
    let timing = std::env::args().any(|s| s == "--timing");
    let mut r = gpu::Renderer::from_adapter_for_diagnostics(
        adapter,
        wgpu::TextureFormat::Rgba8Unorm,
        4,
        timing,
    )
    .await?;
    let scope = r.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut rows = Vec::new();
    for dpr in [1., 1.5] {
        for low in [false, true] {
            for out in [false, true] {
                let (mut g, physical) = scene(out, dpr)?;
                let extent = g.window_viewport(physical, dpr)?;
                let mut world = g.client().clone();
                let mut quality = g.quality();
                quality.requested = if low {
                    QualityMode::Low
                } else {
                    QualityMode::High
                };
                quality.high_quality = !low;
                quality.quality_blend = if low { 0. } else { 1. };
                quality.quality_scale = if low { 1.7 } else { 1. };
                let picture =
                    render::prepare_gameplay(&mut world, &mut g.bank, extent, 1., 999, quality)?;
                r.sync_resources(&[])?;
                r.sync_resources(&picture.resources)?;
                let mut p = WorldPresenter::new(&r);
                let target = support::offscreen::target(&r, physical);
                for mode in [WorldResolution::Original, WorldResolution::Native] {
                    let report = p.render(
                        &mut r,
                        &picture.frame,
                        None,
                        &target.create_view(&Default::default()),
                        physical,
                        mode,
                    )?;
                    let image = image::RgbaImage::from_raw(
                        physical[0],
                        physical[1],
                        support::offscreen::pixels(&r, &target)?,
                    )
                    .ok_or("image length")?;
                    let file = format!(
                        "{mode:?}-{}-{}-dpr{dpr}.png",
                        if low { "low" } else { "high" },
                        if out { "out" } else { "default" }
                    );
                    image.save(output.join(&file)).map_err(|e| e.to_string())?;
                    std::fs::write(
                        output.join(file.replace(".png", ".draws.txt")),
                        format!("{:?}", picture.frame),
                    )
                    .map_err(|e| e.to_string())?;
                    if timing && dpr == 1. {
                        support::pellet_timing::run(
                            &mut r,
                            &mut p,
                            &picture,
                            &g.bank,
                            &target.create_view(&Default::default()),
                            physical,
                            mode,
                            &output.join(file.replace(".png", ".timing.json")),
                        )?;
                    }
                    rows.push(serde_json::json!({"file":file,"dpr":dpr,"physical":physical,"projection":extent,"attachment":report.plan.attachment,"samples":report.plan.samples,"draws":picture.frame.draws.len(),"gpu_draws":report.world_draws,"texture_bytes":picture.pixel_bytes(),"quality":format!("{quality:?}")}));
                }
            }
        }
    }
    if let Some(error) = scope.pop().await {
        return Err(error.to_string());
    }
    std::fs::write(output.join("matrix.json"), serde_json::to_vec_pretty(&serde_json::json!({"fixture":"authored 80-food scene; same frozen clock, physical extent and camera inputs on both sides","adapter":format!("{:?}",r.adapter_info),"rows":rows})).unwrap()).map_err(|e| e.to_string())?;
    Ok(())
}
fn main() {
    let output = std::env::args_os().nth(1).expect("new output directory");
    if let Err(error) = pollster::block_on(run(Path::new(&output))) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
