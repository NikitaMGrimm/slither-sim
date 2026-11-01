//! Authored synthetic offscreen comparison using the production presenter.
mod support {
    pub mod offscreen;
}
use slither_client::settings::WorldResolution::{Native, Original};
use slither_wgpu_slice::{
    Result,
    gpu::{Renderer, instance},
    world_presenter::WorldPresenter,
};
use std::{path::Path, time::Instant};

async fn run(output: &Path) -> Result<()> {
    std::fs::create_dir(output).map_err(|e| e.to_string())?;
    let mut measurements = Vec::new();
    let mut adapters = Vec::new();
    for samples in [4, 1] {
        let instance = instance();
        let adapter = if std::env::args().any(|arg| arg == "--integrated") {
            instance
                .enumerate_adapters(wgpu::Backends::DX12)
                .await
                .into_iter()
                .find(|a| a.get_info().device_type == wgpu::DeviceType::IntegratedGpu)
                .ok_or("No DX12 integrated GPU")?
        } else {
            instance
                .request_adapter(&Default::default())
                .await
                .map_err(|e| e.to_string())?
        };
        let mut renderer = Renderer::from_adapter_with_max_samples(
            adapter,
            wgpu::TextureFormat::Rgba8Unorm,
            samples,
        )
        .await?;
        assert_eq!(
            renderer.sample_count, samples,
            "requested sample-path evidence unavailable"
        );
        adapters.push(serde_json::json!({"samples":samples,"adapter":format!("{:?}",renderer.adapter_info),"negotiated_max_dimension":renderer.device.limits().max_texture_dimension_2d}));
        let scope = renderer
            .device
            .push_error_scope(wgpu::ErrorFilter::Validation);
        for physical in [[1920, 1080], [2560, 1440], [3840, 2160]] {
            let preparation = Instant::now();
            let (world, overlay, resources) = support::offscreen::scene(physical);
            let prepare_us = preparation.elapsed().as_micros();
            let sync = Instant::now();
            renderer.sync_resources(&resources)?;
            let sync_us = sync.elapsed().as_micros();
            let mut presenter = WorldPresenter::new(&renderer);
            let target = support::offscreen::target(&renderer, physical);
            let projection = world.clone();
            let mut overlay_pixels = None;
            for mode in [Original, Native] {
                let start = Instant::now();
                let report = presenter.render(
                    &mut renderer,
                    &world,
                    Some(&overlay),
                    &target.create_view(&Default::default()),
                    physical,
                    mode,
                )?;
                let first_submit_us = start.elapsed().as_micros();
                assert_eq!(world, projection);
                assert_eq!(report.plan.effective, mode);
                assert_eq!(presenter.backing_extent().is_some(), mode == Original);
                let mut warm_submit_us = Vec::new();
                for _ in 0..5 {
                    let start = Instant::now();
                    presenter.render(
                        &mut renderer,
                        &world,
                        Some(&overlay),
                        &target.create_view(&Default::default()),
                        physical,
                        mode,
                    )?;
                    warm_submit_us.push(start.elapsed().as_micros());
                }
                let start = Instant::now();
                let pixels = support::offscreen::pixels(&renderer, &target)?;
                let wait_copy_us = start.elapsed().as_micros();
                let image = image::RgbaImage::from_raw(physical[0], physical[1], pixels)
                    .ok_or("image length")?;
                let marker = image.get_pixel(32, 20).0;
                if let Some(previous) = overlay_pixels {
                    assert_eq!(marker, previous);
                } else {
                    overlay_pixels = Some(marker);
                }
                let name = if mode == Original {
                    "original"
                } else {
                    "native"
                };
                let file = format!(
                    "{name}-{}x{}-max-msaa{samples}-world-msaa{}.png",
                    physical[0], physical[1], report.plan.samples
                );
                image.save(output.join(&file)).map_err(|e| e.to_string())?;
                let crop = [physical[0] / 2 - 192, physical[1] / 2 - 128, 384, 256];
                image::imageops::crop_imm(&image, crop[0], crop[1], crop[2], crop[3])
                    .to_image()
                    .save(output.join(format!("edge-{file}")))
                    .map_err(|e| e.to_string())?;
                measurements.push(serde_json::json!({"file":file,"physical":physical,"projection":world.extent,"attachment":report.plan.attachment,"maximum_samples":samples,"world_samples":report.plan.samples,"world_producer_quads":world.draws.len(),"overlay_producer_quads":overlay.draws.len(),"world_gpu_draws":report.world_draws,"overlay_gpu_draws":report.overlay_draws,"blit_draws":usize::from(report.plan.blit),"render_passes":report.passes,"backing_bytes":report.plan.backing_bytes(),"msaa_bytes":report.plan.msaa_bytes(),"persistent_world_target_bytes":report.target_bytes,"transition_world_target_peak_bytes":report.transient_target_bytes,"one_presentation_image_bytes":u64::from(physical[0])*u64::from(physical[1])*4,"uploaded_resource_slots":resources.len(),"uploaded_resource_bytes":resources.iter().map(|r|r.image.pixels.len()).sum::<usize>(),"prepare_cpu_us":prepare_us,"sync_cpu_us":sync_us,"first_submit_cpu_us":first_submit_us,"warm_submit_cpu_us":warm_submit_us,"copy_wait_readback_cpu_us":wait_copy_us,"edge_crop":crop}));
            }
        }
        if let Some(error) = scope.pop().await {
            return Err(error.to_string());
        }
    }
    let revision = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|e| e.to_string())?;
    let manifest = serde_json::json!({"revision":String::from_utf8_lossy(&revision.stdout).trim(),"fixture":"authored synthetic opaque geometry; no external assets or servers","adapters":adapters,"measurements":measurements,"limits":"Logical target-byte accounting includes old plus replacement at transition. Driver padding, swapchain multiplicity, vertex buffers and upload staging are excluded. Readback waits are separate from CPU submission; no GPU timestamps, FPS, power or weak-GPU claim. Offscreen presentation images stand in for acquired surface images."});
    std::fs::write(
        output.join("world-resolution.json"),
        serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
fn main() {
    let output = std::env::args_os().nth(1).expect("new output directory");
    if let Err(error) = pollster::block_on(run(Path::new(&output))) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
