//! Finite opt-in zoom workload. CPU picture build, logical draws and actual GPU
//! draw calls are separate measurements; GPU waits/readbacks are never timed.
#[path = "../../tests/support/zoom_world.rs"]
mod synthetic;
use super::*;
use std::fmt::Write;

pub fn run(output: &Path) -> Result<()> {
    std::fs::create_dir(output).map_err(|e| e.to_string())?;
    let mut game = synthetic::world()?;
    let quality = game.quality();
    let hud = game.hud_view();
    let instance = gpu::instance();
    let adapter = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::DX12))
        .into_iter()
        .find(|a| a.get_info().device_type != wgpu::DeviceType::Cpu)
        .ok_or("no hardware adapter exposed")?;
    let mut renderer = pollster::block_on(gpu::Renderer::from_adapter(
        adapter,
        wgpu::TextureFormat::Rgba8Unorm,
    ))?;
    println!(
        "adapter={:?}, samples={}",
        renderer.adapter_info, renderer.sample_count
    );
    let scope = renderer
        .device
        .push_error_scope(wgpu::ErrorFilter::Validation);
    let texture = target(&renderer);
    let view = texture.create_view(&Default::default());
    let mut worlds: Vec<_> = [0., 1000., -17., -1000.]
        .into_iter()
        .map(|notches| {
            let mut world = game.client().clone();
            world.wheel_zoom(notches);
            world
        })
        .collect();
    let labels = ["default", "in", "mid", "out"];
    let mut identity = 10000;
    // Settle interpolation before measuring; all modes retain the same frozen
    // synthetic entities, animation state, stock scale, viewport and quality.
    for world in &mut worlds {
        for _ in 0..110 {
            identity += 1;
            let p = render::prepare_gameplay(
                world,
                &mut game.bank,
                synthetic::EXTENT,
                1.,
                identity,
                quality,
            )?;
            game.bank.recycle_frame(p);
        }
    }
    let mut csv = String::from(
        "round,level,frame,scale,draws,gpu_draw_calls,prepare_ns,pixels,draw_capacity\n",
    );
    // Five alternating rounds, 20 warm then 40 measured frames per level.
    for round in 0..5 {
        let order = if round % 2 == 0 {
            [0, 1, 2, 3]
        } else {
            [3, 2, 1, 0]
        };
        for level in order {
            for frame in 0..60 {
                identity += 1;
                let start = Instant::now();
                let p = render::prepare_gameplay(
                    &mut worlds[level],
                    &mut game.bank,
                    synthetic::EXTENT,
                    1.,
                    identity,
                    quality,
                )?;
                let p = render::finish_hud(p, &mut game.bank, &hud)?;
                let ns = start.elapsed().as_nanos();
                if p.draw_count() > slither_wgpu_slice::frame::MAX_DRAWS {
                    return Err("draw budget exceeded".into());
                }
                renderer.sync_resources(&p.resources)?;
                renderer.render(&p.frame, &view)?;
                let mut calls = renderer.draw_call_count();
                if let Some(overlay) = &p.overlay {
                    renderer.render_overlay(overlay, &view)?;
                    calls += renderer.draw_call_count();
                }
                wait(&renderer)?;
                if frame >= 20 {
                    writeln!(
                        csv,
                        "{round},{},{frame},{:.12},{},{calls},{ns},{},{}",
                        labels[level],
                        worlds[level].camera().zoom(),
                        p.draw_count(),
                        p.pixel_bytes(),
                        p.frame.draws.capacity()
                    )
                    .unwrap();
                }
                if round == 0 && frame == 59 {
                    let raw = output.join(format!("{}.rgba", labels[level]));
                    capture(&renderer, &texture, &raw)?;
                    let bytes = std::fs::read(&raw).map_err(|e| e.to_string())?;
                    let file = std::fs::File::create(output.join(format!("{}.png", labels[level])))
                        .map_err(|e| e.to_string())?;
                    let mut encoder = png::Encoder::new(file, 960, 540);
                    encoder.set_color(png::ColorType::Rgba);
                    encoder.set_depth(png::BitDepth::Eight);
                    encoder
                        .write_header()
                        .map_err(|e| e.to_string())?
                        .write_image_data(&bytes)
                        .map_err(|e| e.to_string())?;
                }
                game.bank.recycle_frame(p);
            }
            println!("round={round} level={} complete", labels[level]);
        }
    }
    if let Some(e) = pollster::block_on(scope.pop()) {
        return Err(format!("GPU validation: {e}"));
    }
    std::fs::write(output.join("zoom.csv"), csv).map_err(|e| e.to_string())?;
    println!("PASS: 5 rounds, 200 measured pictures per zoom level, zero rejected pictures");
    Ok(())
}
