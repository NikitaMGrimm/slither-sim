//! Shared surface/offscreen world presentation. Projection data is borrowed unchanged.
use crate::{
    frame::Frame,
    gpu::{RenderError, Renderer},
    world_resolution::WorldTargetPlan,
};
use slither_client::settings::WorldResolution;

#[derive(Clone, Copy, Debug)]
pub struct PresentationReport {
    pub plan: WorldTargetPlan,
    pub world_draws: usize,
    pub overlay_draws: usize,
    pub passes: usize,
    /// Host-owned world targets, excluding the presentation image and uploaded art.
    pub target_bytes: u64,
    /// Old and replacement targets may coexist during submission.
    pub transient_target_bytes: u64,
}

pub struct WorldPresenter {
    backing: Option<wgpu::Texture>,
    blitter: wgpu::util::TextureBlitter,
    effective: WorldResolution,
}
impl WorldPresenter {
    pub fn new(renderer: &Renderer) -> Self {
        Self {
            backing: None,
            blitter: wgpu::util::TextureBlitterBuilder::new(&renderer.device, renderer.format)
                .sample_type(wgpu::FilterMode::Linear)
                .build(),
            effective: WorldResolution::Original,
        }
    }
    pub fn effective(&self) -> WorldResolution {
        self.effective
    }
    pub fn backing_extent(&self) -> Option<[u32; 2]> {
        self.backing.as_ref().map(|t| [t.width(), t.height()])
    }
    pub fn render(
        &mut self,
        renderer: &mut Renderer,
        frame: &Frame,
        overlay: Option<&Frame>,
        target: &wgpu::TextureView,
        physical: [u32; 2],
        requested: WorldResolution,
    ) -> Result<PresentationReport, RenderError> {
        if let Some(overlay) = overlay {
            if overlay.extent != physical {
                return Err("overlay/surface extent mismatch".into());
            }
            renderer.validate_frame(overlay)?;
            crate::world_resolution::validate_target(physical, 1, crate::frame::MAX_EXTENT)?;
        }
        let plan = renderer.world_plan(requested, frame.extent, physical)?;
        let old_backing = self.backing_extent();
        let old_msaa = renderer.multisample_extent();
        let layer_extents = |r: &Renderer| {
            [
                r.food_outline_extent(),
                r.half_glow_extent(),
                r.fine_glow_extent(),
                r.food_glow_extent(),
            ]
        };
        let bytes = |e: Option<[u32; 2]>| e.map_or(0, |e| u64::from(e[0]) * u64::from(e[1]) * 4);
        let old_layers = layer_extents(renderer);
        let old_msaa_bytes = old_msaa.map_or(0, |(e, s)| {
            u64::from(e[0]) * u64::from(e[1]) * 4 * u64::from(s)
        });
        let old_backing_bytes = old_backing.map_or(0, |e| u64::from(e[0]) * u64::from(e[1]) * 4);
        let replacement = if plan.blit && old_backing != Some(plan.attachment) {
            Some(renderer.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("desktop policy backing"),
                size: wgpu::Extent3d {
                    width: plan.attachment[0],
                    height: plan.attachment[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: renderer.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            }))
        } else {
            None
        };
        let mut transient_target_bytes = old_msaa_bytes
            + old_backing_bytes
            + if old_msaa == Some((plan.attachment, plan.samples)) {
                0
            } else {
                plan.msaa_bytes()
            }
            + if replacement.is_some() {
                plan.backing_bytes()
            } else {
                0
            };
        let world_draws;
        let world_passes;
        if plan.blit {
            let backing = replacement
                .as_ref()
                .or(self.backing.as_ref())
                .expect("prepared backing")
                .create_view(&Default::default());
            renderer.render_world_at(frame, &backing, plan.attachment, plan.samples)?;
            world_draws = renderer.draw_call_count();
            world_passes = renderer.world_pass_count();
            let mut encoder = renderer.device.create_command_encoder(&Default::default());
            #[cfg(feature = "frame-profile")]
            let timing = renderer.begin_diagnostic_encoder_pass(&mut encoder, "blit")?;
            self.blitter
                .copy(&renderer.device, &mut encoder, &backing, target);
            #[cfg(feature = "frame-profile")]
            renderer.end_diagnostic_encoder_pass(&mut encoder, timing);
            let submission = crate::latency::submit_begin();
            renderer.queue.submit([encoder.finish()]);
            crate::latency::submit_end(submission);
        } else {
            renderer.render_world_at(frame, target, plan.attachment, plan.samples)?;
            world_draws = renderer.draw_call_count();
            world_passes = renderer.world_pass_count();
        }
        let new_layers = layer_extents(renderer);
        let layer_bytes = new_layers.into_iter().map(bytes).sum::<u64>();
        transient_target_bytes += old_layers
            .into_iter()
            .zip(new_layers)
            .map(|(old, new)| bytes(old) + if old == new { 0 } else { bytes(new) })
            .sum::<u64>();
        let overlay_draws = if let Some(overlay) = overlay {
            renderer.render_overlay(overlay, target)?;
            renderer.draw_call_count()
        } else {
            0
        };
        // Publish only after all work submits. wgpu retains resources referenced by
        // prior submissions, so retirement never destroys an in-flight attachment.
        if let Some(replacement) = replacement {
            self.backing = Some(replacement);
        }
        if plan.effective == WorldResolution::Native {
            self.backing = None;
        }
        self.effective = plan.effective;
        renderer.effective_sample_count = plan.samples;
        let target_bytes = layer_bytes
            + renderer.multisample_extent().map_or(0, |(e, s)| {
                u64::from(e[0]) * u64::from(e[1]) * 4 * u64::from(s)
            })
            + self
                .backing_extent()
                .map_or(0, |e| u64::from(e[0]) * u64::from(e[1]) * 4);
        Ok(PresentationReport {
            plan,
            world_draws,
            overlay_draws,
            passes: world_passes + usize::from(plan.blit) + usize::from(overlay.is_some()),
            target_bytes,
            transient_target_bytes,
        })
    }
}
