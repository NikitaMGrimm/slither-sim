//! Matched resource control: the old single-level food rasters, same candidate renderer.
use slither_client::{authored_sources::authored_source_recipes, settings::WorldResolution};
use slither_wgpu_slice::{
    Result, bank::Bank, gpu::Renderer, materials::materialize, render::PreparedFrame,
    world_presenter::WorldPresenter,
};
use std::{collections::BTreeMap, path::Path};

fn elapsed(r: &Renderer) -> Result<f64> {
    let buffer = |usage| {
        r.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("finite pellet timestamp readback"),
            size: 256,
            usage,
            mapped_at_creation: false,
        })
    };
    let resolve = buffer(wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC);
    let read = buffer(wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ);
    let mut encoder = r.device.create_command_encoder(&Default::default());
    encoder.resolve_query_set(
        r.diagnostic_timestamps().ok_or("timestamps unavailable")?,
        0..2,
        &resolve,
        0,
    );
    encoder.copy_buffer_to_buffer(&resolve, 0, &read, 0, 16);
    r.queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    read.slice(..).map_async(wgpu::MapMode::Read, move |v| {
        let _ = tx.send(v);
    });
    r.device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(30)),
        })
        .map_err(|e| e.to_string())?;
    rx.recv()
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let data = read
        .slice(..)
        .get_mapped_range()
        .map_err(|e| e.to_string())?;
    let start = u64::from_le_bytes(data[0..8].try_into().unwrap());
    let end = u64::from_le_bytes(data[8..16].try_into().unwrap());
    let ns = end.checked_sub(start).ok_or("timestamp order")? as f64
        * f64::from(r.queue.get_timestamp_period());
    drop(data);
    read.unmap();
    Ok(ns)
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    r: &mut Renderer,
    p: &mut WorldPresenter,
    picture: &PreparedFrame,
    bank: &Bank,
    target: &wgpu::TextureView,
    physical: [u32; 2],
    mode: WorldResolution,
    output: &Path,
) -> Result<()> {
    let mut old = BTreeMap::new();
    for recipes in authored_source_recipes().food {
        let sources = bank.food(recipes.color as u8, [0., 0.]);
        for (source, recipe) in sources
            .bodies
            .iter()
            .zip(&recipes.bodies)
            .chain(sources.glows.iter().zip(&recipes.glows))
        {
            old.insert(
                source.asset().handle,
                std::sync::Arc::new(materialize(recipe)?),
            );
        }
    }
    let mut control = picture.resources.clone();
    for resource in &mut control {
        if let Some(image) = old.get(&resource.asset.handle) {
            resource.image = image.clone();
        }
    }
    let mut rows = Vec::new();
    for round in 0..3 {
        for candidate in if round % 2 == 0 {
            [false, true]
        } else {
            [true, false]
        } {
            r.sync_resources(&[])?;
            r.sync_resources(if candidate {
                &picture.resources
            } else {
                &control
            })?;
            for frame in 0..30 {
                let report = p.render(r, &picture.frame, None, target, physical, mode)?;
                let ns = elapsed(r)?;
                if frame >= 10 {
                    rows.push(serde_json::json!({"round":round,"candidate":candidate,"frame":frame,"gpu_world_ns":ns,"gpu_draws":report.world_draws}));
                }
            }
        }
    }
    r.sync_resources(&[])?;
    r.sync_resources(&picture.resources)?;
    std::fs::write(output, serde_json::to_vec_pretty(&serde_json::json!({
        "control":"matched resource configuration: original one-level food gradients; same renderer and frame",
        "measurement":"GPU timestamps within world render pass; excludes upload, blit, HUD, readback and CPU preparation; three alternating pairs, 10 warmup + 20 measured frames each; all samples retained",
        "rows":rows,
    })).unwrap()).map_err(|e| e.to_string())
}
