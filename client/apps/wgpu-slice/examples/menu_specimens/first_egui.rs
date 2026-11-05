//! Authored menu-only cold/warm samples. No window, network or external art.
use super::*;
use slither_wgpu_slice::login_native::NativeLogin;
use std::time::Instant;

fn wait(renderer: &Renderer) -> Result<()> {
    renderer
        .device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(Duration::from_secs(20)),
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn run(output: &Path) -> Result<()> {
    let backend = std::env::args().nth(3).unwrap_or("vulkan".into());
    if !["vulkan", "dx12"].contains(&backend.as_str()) {
        return Err("--first-egui backend must be vulkan or dx12".into());
    }
    let screen_arg = std::env::args().nth(4).unwrap_or("login".into());
    if !["login", "settings", "ingame", "picker"].contains(&screen_arg.as_str()) {
        return Err("--first-egui screen must be login, settings, ingame or picker".into());
    }
    let startup = Instant::now();
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: if backend == "dx12" {
            wgpu::Backends::DX12
        } else {
            wgpu::Backends::VULKAN
        },
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: if backend == "dx12" {
                wgpu::PowerPreference::LowPower
            } else {
                wgpu::PowerPreference::HighPerformance
            },
            ..Default::default()
        })
        .await
        .map_err(|e| e.to_string())?;
    let mut renderer = Renderer::from_adapter(adapter, wgpu::TextureFormat::Rgba8Unorm).await?;
    let gpu_ms = startup.elapsed().as_secs_f64() * 1000.;
    let validation = renderer
        .device
        .push_error_scope(wgpu::ErrorFilter::Validation);
    let mut rows = Vec::new();
    let screen = screen_arg.as_str();
    let began = Instant::now();
    let mut login = NativeLogin::new("Authored Player")?;
    login.step(9600., true, false)?;
    match screen {
        "settings" => {
            login.open_settings(true);
            login.step(10000., true, false)?;
        }
        "picker" => {
            login.open_picker();
            login.picker.policy.alpha = 1.;
        }
        _ => {}
    }
    if screen == "ingame" {
        login.egui.set_active(true);
        for _ in 0..2 {
            login.egui.frame([960, 540], 1., &[], |ctx, reveal| {
                login.ingame_ui.show(ctx, true, 1., reveal);
            });
        }
    }
    let init_ms = began.elapsed().as_secs_f64() * 1000.;
    for frame in 0..6 {
        let began = Instant::now();
        let picture = if screen == "ingame" {
            login.egui.set_active(true);
            login.egui.frame([960, 540], 1., &[], |ctx, reveal| {
                login.ingame_ui.show(ctx, true, 1., reveal);
            });
            MenuFrame::world_crossfade([960, 540], 1., 0.)?
        } else {
            login.prepare_host([960, 540], 1., true, None)?
        };
        let prepare_ms = began.elapsed().as_secs_f64() * 1000.;
        let frame_data = page_frame(&picture);
        // Use the existing authored render/readback harness, with an exclusive
        // paint + GPU completion span inside its usual layer callback.
        let mut paint_ms = 0.;
        let pixels = readback_with(&mut renderer, &frame_data, &picture.resources, |r, v| {
            let began = Instant::now();
            login.egui.paint(r, v);
            wait(r).unwrap();
            paint_ms = began.elapsed().as_secs_f64() * 1000.;
        })?;
        let first_picture_ms = startup.elapsed().as_secs_f64() * 1000.;
        if frame == 0 || frame == 5 {
            image::save_buffer(
                output.join(format!("{screen}-{frame}.png")),
                &pixels,
                960,
                540,
                image::ColorType::Rgba8,
            )
            .map_err(|e| e.to_string())?;
        }
        rows.push(
            serde_json::json!({"screen":screen,"frame":frame,"init_ms":init_ms,
            "prepare_ms":prepare_ms,"paint_ms":paint_ms,
            "first_picture_ms":(frame == 0).then_some(first_picture_ms)}),
        );
    }
    if let Some(e) = validation.pop().await {
        return Err(e.to_string());
    }
    std::fs::write(
        output.join("timing.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "adapter":format!("{:?}", renderer.adapter_info),"gpu_ms":gpu_ms,"rows":rows
        }))
        .unwrap(),
    )
    .map_err(|e| e.to_string())
}
