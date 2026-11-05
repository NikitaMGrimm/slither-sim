//! The same ordered quad commands for direct passes and native render bundles.
use super::{DrawState, Pipeline, Renderer, draw_runs};
use crate::frame::VERTEX_CHUNK_QUADS;
use std::ops::Range;

trait QuadEncoder<'a> {
    fn indices(&mut self, buffer: wgpu::BufferSlice<'a>);
    fn vertices(&mut self, buffer: wgpu::BufferSlice<'a>);
    fn pipeline(&mut self, pipeline: &'a wgpu::RenderPipeline);
    fn source(&mut self, bind: &'a wgpu::BindGroup);
    fn draw(&mut self, vertices: Range<u32>);
}
impl<'a> QuadEncoder<'a> for wgpu::RenderPass<'a> {
    fn indices(&mut self, buffer: wgpu::BufferSlice<'a>) {
        self.set_index_buffer(buffer, wgpu::IndexFormat::Uint16);
    }
    fn vertices(&mut self, buffer: wgpu::BufferSlice<'a>) {
        self.set_vertex_buffer(0, buffer);
    }
    fn pipeline(&mut self, pipeline: &'a wgpu::RenderPipeline) {
        self.set_pipeline(pipeline);
    }
    fn source(&mut self, bind: &'a wgpu::BindGroup) {
        self.set_bind_group(0, bind, &[]);
    }
    fn draw(&mut self, vertices: Range<u32>) {
        self.draw_indexed(vertices, 0, 0..1);
    }
}
#[cfg(not(target_arch = "wasm32"))]
impl<'a> QuadEncoder<'a> for wgpu::RenderBundleEncoder<'a> {
    fn indices(&mut self, buffer: wgpu::BufferSlice<'a>) {
        self.set_index_buffer(buffer, wgpu::IndexFormat::Uint16);
    }
    fn vertices(&mut self, buffer: wgpu::BufferSlice<'a>) {
        self.set_vertex_buffer(0, buffer);
    }
    fn pipeline(&mut self, pipeline: &'a wgpu::RenderPipeline) {
        self.set_pipeline(pipeline);
    }
    fn source(&mut self, bind: &'a wgpu::BindGroup) {
        self.set_bind_group(0, bind, &[]);
    }
    fn draw(&mut self, vertices: Range<u32>) {
        self.draw_indexed(vertices, 0, 0..1);
    }
}

impl Renderer {
    pub(super) fn encode_ordered<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        range: Range<usize>,
        samples: u32,
        filter: Option<u8>,
    ) -> usize {
        #[cfg(not(target_arch = "wasm32"))]
        if self.canonical_bundles {
            let ranges: Vec<_> = self
                .vertices
                .iter()
                .enumerate()
                .filter_map(|(i, chunk)| {
                    let base = i * VERTEX_CHUNK_QUADS;
                    let start = range.start.max(base);
                    let end = range.end.min(base + chunk.states.len());
                    (start < end).then_some(start..end)
                })
                .collect();
            if ranges.len() >= 4 {
                return self.encode_bundles(pass, &ranges, samples, filter);
            }
        }
        self.encode_quads(pass, range, samples, filter)
    }

    fn encode_quads<'a>(
        &'a self,
        pass: &mut impl QuadEncoder<'a>,
        range: Range<usize>,
        samples: u32,
        filter: Option<u8>,
    ) -> usize {
        let mut calls = 0;
        let mut previous: Option<DrawState> = None;
        pass.indices(self.indices.slice(..));
        for (i, chunk) in self.vertices.iter().enumerate() {
            let base = i * VERTEX_CHUNK_QUADS;
            let start = range.start.saturating_sub(base).min(chunk.states.len());
            let end = range.end.saturating_sub(base).min(chunk.states.len());
            if start >= end {
                continue;
            }
            pass.vertices(chunk.buffer.as_ref().expect("vertex buffer").slice(..));
            for (state, vertices) in draw_runs(chunk.states[start..end].iter().copied()) {
                if state.noop || filter.is_some_and(|scale| scale != state.glow_scale) {
                    continue;
                }
                if previous.is_none_or(|p| p.pipeline != state.pipeline) {
                    pass.pipeline(match state.pipeline {
                        Pipeline::BlackOver => {
                            if samples == 1 {
                                &self.overlay_black_over
                            } else {
                                &self.black_over[&samples]
                            }
                        }
                        Pipeline::PairLighter => {
                            if let Some(scale) = filter.filter(|&s| s > 0) {
                                &self.glow_pair_lighter[scale as usize - 1]
                            } else if samples == 1 {
                                &self.overlay_pair_lighter
                            } else {
                                &self.pair_lighter[&samples]
                            }
                        }
                        Pipeline::Lighter => {
                            if let Some(scale) = filter.filter(|&s| s > 0) {
                                &self.glow_lighter[scale as usize - 1]
                            } else if samples == 1 {
                                &self.overlay_lighter
                            } else {
                                &self.lighter[&samples]
                            }
                        }
                        Pipeline::Over => {
                            if samples == 1 {
                                &self.overlay_over
                            } else {
                                &self.over[&samples]
                            }
                        }
                    });
                }
                if previous.is_none_or(|p| (p.key, p.repeat) != (state.key, state.repeat)) {
                    let source = &self.bindings[&state.key];
                    pass.source(if state.repeat {
                        &source.repeat_bind
                    } else {
                        &source.bind
                    });
                }
                pass.draw(vertices.start + start as u32 * 6..vertices.end + start as u32 * 6);
                calls += 1;
                previous = Some(state);
            }
        }
        calls
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn encode_bundles<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        ranges: &[Range<usize>],
        samples: u32,
        filter: Option<u8>,
    ) -> usize {
        use std::sync::{
            OnceLock,
            atomic::{AtomicUsize, Ordering},
        };
        let results: Vec<_> = (0..ranges.len()).map(|_| OnceLock::new()).collect();
        let next = AtomicUsize::new(0);
        let record = |i: usize| {
            // Construct and finish on the same thread: wgpu encoders are !Send.
            let mut bundle =
                self.device
                    .create_render_bundle_encoder(&wgpu::RenderBundleEncoderDescriptor {
                        label: Some("canonical quad chunk"),
                        color_formats: &[Some(self.format)],
                        depth_stencil: None,
                        sample_count: samples,
                        multiview: None,
                    });
            let count = self.encode_quads(&mut bundle, ranges[i].clone(), samples, filter);
            (
                bundle.finish(&wgpu::RenderBundleDescriptor {
                    label: Some("ordered canonical quad chunk"),
                }),
                count,
            )
        };
        let drain = || loop {
            let i = next.fetch_add(1, Ordering::Relaxed);
            if i >= ranges.len() {
                break;
            }
            let _ = results[i].set(record(i));
        };
        std::thread::scope(|scope| {
            let participants = std::thread::available_parallelism().map_or(1, |n| n.get().min(4));
            let mut workers = Vec::new();
            for _ in 1..participants {
                let drain = &drain;
                if let Ok(worker) = std::thread::Builder::new().spawn_scoped(scope, drain) {
                    workers.push(worker);
                }
            }
            drain();
            for worker in workers {
                let _ = worker.join();
            }
        });
        // Preserve chunk order, including any work a failed helper did not publish.
        for (i, result) in results.iter().enumerate() {
            if result.get().is_none() {
                let _ = result.set(record(i));
            }
        }
        pass.execute_bundles(results.iter().map(|r| &r.get().unwrap().0));
        results.iter().map(|r| r.get().unwrap().1).sum()
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::pellet_batch::offscreen;

    #[test]
    #[ignore = "explicit authored offscreen ordered bundle/direct comparison"]
    fn partial_chunk_bundles_preserve_pixels_and_call_counts() {
        for (backend, vendor) in [
            (wgpu::Backends::VULKAN, 0x10de),
            (wgpu::Backends::DX12, 0x1002),
        ] {
            let adapter = pollster::block_on(super::super::instance().enumerate_adapters(backend))
                .into_iter()
                .find(|a| a.get_info().vendor == vendor)
                .expect("requested hardware adapter");
            let mut r = pollster::block_on(Renderer::from_adapter_with_max_samples(
                adapter,
                wgpu::TextureFormat::Rgba8Unorm,
                1,
            ))
            .unwrap();
            let (mut frame, _, resources) = offscreen::scene([128, 96]);
            let template = frame.draws[0].clone();
            frame.draws = (0..5 * VERTEX_CHUNK_QUADS + 7)
                .map(|i| {
                    let mut draw = template.clone();
                    draw.asset = resources[i % resources.len()].asset;
                    draw.destination = slither_client::presentation::BodyRect {
                        x: (i % 127) as f64 + 0.125,
                        y: (i % 95) as f64 + 0.25,
                        width: 9.,
                        height: 7.,
                    };
                    draw.effective_alpha = 0.01 * (1 + i % 7) as f64;
                    draw.blend = if i.is_multiple_of(3) {
                        slither_client::presentation::BodyBlend::Lighter
                    } else {
                        slither_client::presentation::BodyBlend::SourceOver
                    };
                    draw
                })
                .collect();
            r.sync_resources(&resources).unwrap();
            let staging = offscreen::target(&r, frame.extent);
            r.render(&frame, &staging.create_view(&Default::default()))
                .unwrap();
            for range in [
                0..frame.draws.len(),
                VERTEX_CHUNK_QUADS - 1..5 * VERTEX_CHUNK_QUADS - 1,
            ] {
                let mut images = Vec::new();
                let mut counts = Vec::new();
                for bundled in [false, true] {
                    r.canonical_bundles = bundled;
                    let target = offscreen::target(&r, frame.extent);
                    let view = target.create_view(&Default::default());
                    let mut encoder = r.device.create_command_encoder(&Default::default());
                    {
                        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &view,
                                depth_slice: None,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            ..Default::default()
                        });
                        counts.push(r.encode_ordered(&mut pass, range.clone(), 1, None));
                    }
                    r.queue.submit([encoder.finish()]);
                    images.push(offscreen::pixels(&r, &target).unwrap());
                }
                assert!(counts[0] > 10_000);
                assert!(
                    images[0]
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .collect::<std::collections::BTreeSet<_>>()
                        .len()
                        > 8
                );
                assert_eq!(counts[0], counts[1]);
                assert_eq!(images[0], images[1], "{backend:?} {range:?}");
            }
        }
    }
}
