//! Ordered instanced pellet passes using the renderer's existing texture arrays.
use super::{
    DrawState, LayerHistory, LayerTarget, Multisample, Pipeline, RenderResult, Renderer,
    VertexChunk, additive_pair,
    soft_layers::{covered_area, select_glows},
    zero_pair,
};
use crate::{
    frame::{Frame, QUAD_VERTEX_BYTES, ResourceKey, VERTEX_CHUNK_QUADS},
    pellet_batch::{Batch, Instance, Sources, exact_instance, glow_pair},
};
use slither_client::presentation::FoodSource;
use std::{ops::Range, sync::Arc};

struct SourceDraw {
    source: FoodSource,
    array: ResourceKey,
    layer: u32,
    bind: wgpu::BindGroup,
}
pub(super) struct Pellets {
    source_layout: wgpu::BindGroupLayout,
    frame_buffer: wgpu::Buffer,
    frame_bind: wgpu::BindGroup,
    body: [wgpu::RenderPipeline; 2],
    full: [wgpu::RenderPipeline; 2],
    reduced: [wgpu::RenderPipeline; 3],
    flags: Option<wgpu::Buffer>,
    dynamic: Option<wgpu::Buffer>,
    sources: Option<Arc<Sources>>,
    mirror: Vec<Instance>,
    bindings: Vec<[Option<SourceDraw>; 3]>,
}
fn uniform(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    size: u64,
) -> (wgpu::Buffer, wgpu::BindGroup) {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("retained pellet uniform"),
        size,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    });
    (buffer, bind)
}
impl Pellets {
    pub fn new(
        device: &wgpu::Device,
        source: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
        samples: u32,
    ) -> Self {
        let uniform_layout = || {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("pellet vertex uniform"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            })
        };
        let frame_layout = uniform_layout();
        let source_layout = uniform_layout();
        let (frame_buffer, frame_bind) = uniform(device, &frame_layout, 16);
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("retained pellets; WebGL2 limits"),
            bind_group_layouts: &[Some(source), Some(&frame_layout), Some(&source_layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("retained pellet expansion"),
            source: wgpu::ShaderSource::Wgsl(
                format!(
                    "{}\n{}",
                    include_str!("quad.wgsl"),
                    include_str!("pellet.wgsl")
                )
                .into(),
            ),
        });
        let dynamic = wgpu::vertex_attr_array![0=>Float32x4,1=>Float32x4,2=>Float32x4,3=>Float32x4,4=>Float32x4,5=>Float32x4];
        let flags = wgpu::vertex_attr_array![6=>Uint32];
        let buffers = [
            Some(wgpu::VertexBufferLayout {
                array_stride: 96,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &dynamic,
            }),
            Some(wgpu::VertexBufferLayout {
                array_stride: 4,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &flags,
            }),
        ];
        let make = |vertex: &str, fragment: &str, samples| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("ordered instanced pellet pair"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(vertex),
                    compilation_options: Default::default(),
                    buffers: &buffers,
                },
                primitive: Default::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: samples,
                    ..Default::default()
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fragment),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState {
                            color: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::One,
                                dst_factor: wgpu::BlendFactor::One,
                                operation: wgpu::BlendOperation::Add,
                            },
                            alpha: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::One,
                                dst_factor: wgpu::BlendFactor::One,
                                operation: wgpu::BlendOperation::Add,
                            },
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let body = [
            make("vs_pellet_body", "fs_pair", 1),
            make("vs_pellet_body", "fs_pair", samples),
        ];
        let full = [
            make("vs_pellet_full", "fs_pair", 1),
            make("vs_pellet_full", "fs_pair", samples),
        ];
        let reduced = [
            make("vs_pellet_half", "fs_pair2", 1),
            make("vs_pellet_fine", "fs_pair4", 1),
            make("vs_pellet_coarse", "fs_pair8", 1),
        ];
        Self {
            source_layout,
            frame_buffer,
            frame_bind,
            body,
            full,
            reduced,
            flags: None,
            dynamic: None,
            sources: None,
            mirror: Vec::new(),
            bindings: Vec::new(),
        }
    }
    fn source(
        r: &Renderer,
        layout: &wgpu::BindGroupLayout,
        s: FoodSource,
        part: u32,
    ) -> RenderResult<SourceDraw> {
        let texture = r
            .textures
            .get(&s.asset().into())
            .ok_or("missing retained pellet source")?;
        let (buffer, bind) = uniform(&r.device, layout, 16);
        let mut bytes = Vec::with_capacity(16);
        for value in [texture.layer, part, 0, 0] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        r.queue.write_buffer(&buffer, 0, &bytes);
        Ok(SourceDraw {
            source: s,
            array: texture.array.key,
            layer: texture.layer,
            bind,
        })
    }
    fn ensure_sources(&mut self, r: &Renderer, b: &Batch) -> RenderResult<()> {
        let changed = self
            .sources
            .as_ref()
            .is_none_or(|s| !Arc::ptr_eq(s, &b.sources));
        let length = b.sources.slots.len();
        if self
            .dynamic
            .as_ref()
            .is_none_or(|b| b.size() < length as u64 * 96)
        {
            self.dynamic = Some(r.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("current exact pellet rectangles and alpha"),
                size: (length as u64 * 96).max(96),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        if self
            .flags
            .as_ref()
            .is_none_or(|x| x.size() < length as u64 * 4)
        {
            self.flags = Some(r.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("pellet visibility and reference glow admission"),
                size: (length as u64 * 4).max(4),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        if changed {
            self.sources = Some(b.sources.clone());
            self.mirror = vec![Instance([[0.; 4]; 6]); length];
        }
        if self.bindings.len() != b.sources.groups.len() {
            self.bindings = (0..b.sources.groups.len())
                .map(|_| [None, None, None])
                .collect();
        }
        for (i, g) in b.sources.groups.iter().enumerate() {
            if !b.visible[g.range.clone()].contains(&true) {
                continue;
            }
            for (part, source) in [g.sources.body, g.sources.inner, g.sources.outer]
                .into_iter()
                .enumerate()
            {
                let t = r
                    .textures
                    .get(&source.asset().into())
                    .ok_or("missing retained pellet source")?;
                if self.bindings[i][part].as_ref().is_none_or(|s| {
                    s.source != source || s.array != t.array.key || s.layer != t.layer
                }) {
                    self.bindings[i][part] =
                        Some(Self::source(r, &self.source_layout, source, part as u32)?);
                }
            }
        }
        Ok(())
    }
    fn prepare(
        &mut self,
        r: &mut Renderer,
        frame: &Frame,
        b: &Batch,
        attachment: [u32; 2],
        samples: u32,
    ) -> RenderResult<[bool; 4]> {
        self.ensure_sources(r, b)?;
        let mut flags = vec![0u32; b.visible.len()];
        let mut coverage = [0.; 4];
        for (i, food) in b.world.foods().iter().enumerate().rev() {
            let slot = b.sources.slots[i];
            if !b.visible[slot] {
                continue;
            }
            let f = food.snapshot();
            let pulse = 0.5 + 0.5 * (f.animation_phase / 13.).cos();
            let glows = [
                glow_pair(frame, f, b.view, false, pulse)?,
                glow_pair(frame, f, b.view, true, pulse)?,
            ];
            let mut bits = 1;
            if zero_pair(f.fade as f32, (pulse * f.fade) as f32) {
                bits |= 32;
            }
            for (outer, (q, second)) in glows.iter().enumerate() {
                let bin = super::glow_bin(q, *second, attachment);
                if bin > 0 {
                    coverage[bin as usize] += covered_area(&q.vertices);
                }
                bits |= u32::from(bin) << if outer == 1 { 3 } else { 1 };
                if zero_pair(q.vertices[0][4], *second) {
                    bits |= if outer == 1 { 128 } else { 64 };
                }
            }
            let exact = exact_instance(frame, f, b.view, &glows, pulse)?;
            self.mirror[slot] = exact;
            flags[slot] = bits;
        }
        let bytes: Vec<_> = self.mirror.iter().flat_map(|i| i.bytes()).collect();
        r.queue
            .write_buffer(self.dynamic.as_ref().expect("dynamic instances"), 0, &bytes);
        let previous = if r.layer_history.extent == attachment && r.layer_history.samples == samples
        {
            r.layer_history
        } else {
            LayerHistory::default()
        };
        let bins = select_glows(coverage, samples, previous);
        let bytes: Vec<_> = flags.iter().flat_map(|f| f.to_le_bytes()).collect();
        r.queue
            .write_buffer(self.flags.as_ref().expect("flags"), 0, &bytes);
        let mut bytes = Vec::with_capacity(16);
        let mask = (1..4).fold(0u32, |m, i| m | if bins[i] { 1 << i } else { 0 });
        for v in [mask, 0, 0, 0] {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        r.queue.write_buffer(&self.frame_buffer, 0, &bytes);
        r.layer_history = LayerHistory {
            extent: attachment,
            samples,
            outline: false,
            glow: bins[1..].contains(&true),
            glow_bins: bins,
        };
        for (i, target) in [&mut r.half_glow, &mut r.fine_glow, &mut r.glow]
            .into_iter()
            .enumerate()
        {
            if bins[i + 1] {
                let factor = 1 << (i + 1);
                let extent = attachment.map(|v| v.div_ceil(factor));
                if target.as_ref().is_none_or(|t| t.extent != extent) {
                    *target = Some(LayerTarget::new(&r.device, &r.layout, r.format, extent));
                }
            } else {
                *target = None;
            }
        }
        Ok(bins)
    }
    fn encode<'a>(
        &'a self,
        r: &'a Renderer,
        pass: &mut wgpu::RenderPass<'a>,
        b: &Batch,
        part: Range<usize>,
    ) -> usize {
        pass.set_bind_group(1, &self.frame_bind, &[]);
        pass.set_index_buffer(r.indices.slice(0..12), wgpu::IndexFormat::Uint16);
        let mut draws = 0;
        for (i, g) in b.sources.groups.iter().enumerate() {
            if !b.visible[g.range.clone()].contains(&true) {
                continue;
            }
            pass.set_vertex_buffer(
                0,
                self.dynamic
                    .as_ref()
                    .expect("dynamic instances")
                    .slice(g.range.start as u64 * 96..g.range.end as u64 * 96),
            );
            pass.set_vertex_buffer(
                1,
                self.flags
                    .as_ref()
                    .expect("flags")
                    .slice(g.range.start as u64 * 4..g.range.end as u64 * 4),
            );
            for p in part.clone() {
                let source = self.bindings[i][p]
                    .as_ref()
                    .expect("visible source binding");
                let texture = &r.textures[&source.source.asset().into()];
                pass.set_bind_group(0, &texture.array.bind, &[]);
                pass.set_bind_group(2, &source.bind, &[]);
                pass.draw_indexed(0..6, 0, 0..g.range.len() as u32);
                draws += 1;
            }
        }
        draws
    }
    #[allow(clippy::too_many_arguments)]
    fn pass(
        &self,
        r: &mut Renderer,
        b: &Batch,
        target: &wgpu::TextureView,
        resolve: Option<&wgpu::TextureView>,
        pipeline: &wgpu::RenderPipeline,
        clear: bool,
        parts: Range<usize>,
        name: &'static str,
    ) -> RenderResult<usize> {
        let mut encoder = r.device.create_command_encoder(&Default::default());
        #[cfg(feature = "frame-profile")]
        let queries = r.diagnostic_pass(name)?;
        #[cfg(not(feature = "frame-profile"))]
        let writes = r
            .diagnostic_timestamps
            .as_ref()
            .map(|set| wgpu::RenderPassTimestampWrites {
                query_set: set,
                beginning_of_pass_write_index: Some(0),
                end_of_pass_write_index: Some(1),
            });
        #[cfg(feature = "frame-profile")]
        let writes = r
            .diagnostic_timestamps
            .as_ref()
            .map(|set| wgpu::RenderPassTimestampWrites {
                query_set: set,
                beginning_of_pass_write_index: Some(queries.0),
                end_of_pass_write_index: Some(queries.1),
            });
        let draws = {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(name),
                timestamp_writes: writes,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: resolve,
                    ops: wgpu::Operations {
                        load: if clear {
                            wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
                        } else {
                            wgpu::LoadOp::Load
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(pipeline);
            self.encode(r, &mut pass, b, parts)
        };
        r.queue.submit([encoder.finish()]);
        Ok(draws)
    }
    pub fn render(
        &mut self,
        r: &mut Renderer,
        frame: &Frame,
        target: &wgpu::TextureView,
        attachment: [u32; 2],
        samples: u32,
    ) -> RenderResult<()> {
        let b = frame.food_batch.as_ref().expect("batch");
        let bins = self.prepare(r, frame, b, attachment, samples)?;
        let ordinary = Frame {
            food_batch: None,
            ..frame.clone()
        };
        let barriers = r.pack_pellet_surroundings(&ordinary, [b.early_at, b.late_at])?;
        if samples > 1
            && r.multisample
                .as_ref()
                .is_none_or(|m| m.extent != attachment || m.samples != samples)
        {
            let texture = r.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("world MSAA"),
                size: wgpu::Extent3d {
                    width: attachment[0],
                    height: attachment[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format: r.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            r.multisample = Some(Multisample {
                extent: attachment,
                samples,
                view: texture.create_view(&Default::default()),
            });
        }
        if samples == 1 {
            r.multisample = None;
        }
        let layers = [&r.half_glow, &r.fine_glow, &r.glow]
            .map(|layer| layer.as_ref().map(|t| (t.view.clone(), t.bind.clone())));
        let mut calls = 0;
        let mut passes = 1;
        for i in 0..3 {
            if let Some(layer) = &layers[i] {
                calls += self.pass(
                    r,
                    b,
                    &layer.0,
                    None,
                    &self.reduced[i],
                    true,
                    1..3,
                    ["food-glows-half", "food-glows-fine", "food-glows-coarse"][i],
                )?;
                passes += 1;
            }
        }
        let mut encoder = r.device.create_command_encoder(&Default::default());
        #[cfg(feature = "frame-profile")]
        let queries = r.diagnostic_pass("world")?;
        #[cfg(not(feature = "frame-profile"))]
        let writes = r
            .diagnostic_timestamps
            .as_ref()
            .map(|set| wgpu::RenderPassTimestampWrites {
                query_set: set,
                beginning_of_pass_write_index: Some(0),
                end_of_pass_write_index: Some(1),
            });
        #[cfg(feature = "frame-profile")]
        let writes = r
            .diagnostic_timestamps
            .as_ref()
            .map(|set| wgpu::RenderPassTimestampWrites {
                query_set: set,
                beginning_of_pass_write_index: Some(queries.0),
                end_of_pass_write_index: Some(queries.1),
            });
        {
            let world = r
                .multisample
                .as_ref()
                .filter(|_| samples > 1)
                .map_or(target, |m| &m.view);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ordered world with retained pellets"),
                timestamp_writes: writes,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: world,
                    depth_slice: None,
                    resolve_target: (samples > 1).then_some(target),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            calls += r.encode_draws(&mut pass, 0..barriers[0], samples, None);
            pass.set_pipeline(&self.body[usize::from(samples > 1)]);
            calls += self.encode(r, &mut pass, b, 0..1);
            calls += r.encode_draws(&mut pass, barriers[0]..barriers[1], samples, None);
            for i in 0..3 {
                if bins[i + 1] {
                    pass.set_pipeline(if i == 2 {
                        if samples == 1 {
                            &r.glow_composite_single
                        } else {
                            &r.glow_composite
                        }
                    } else if samples == 1 {
                        &r.fine_composite_single
                    } else {
                        &r.outline_composite
                    });
                    pass.set_bind_group(0, &layers[i].as_ref().expect("admitted layer").1, &[]);
                    pass.draw(0..3, 0..1);
                    calls += 1;
                }
            }
            pass.set_pipeline(&self.full[usize::from(samples > 1)]);
            calls += self.encode(r, &mut pass, b, 1..3);
            calls += r.encode_draws(&mut pass, barriers[1]..usize::MAX, samples, None);
        }
        let submission = crate::latency::submit_begin();
        r.queue.submit([encoder.finish()]);
        crate::latency::submit_end(submission);
        r.draw_calls = calls;
        r.pass_count = passes;
        Ok(())
    }
}

impl Renderer {
    /// The retained food path has only the original surrounding image stream.
    /// Pack it once so bodies, snakes and glow reconstruction share one MSAA pass.
    fn pack_pellet_surroundings(
        &mut self,
        frame: &Frame,
        barriers: [usize; 2],
    ) -> RenderResult<[usize; 2]> {
        let resident = self
            .textures
            .iter()
            .map(|(key, t)| (*key, t.asset))
            .collect();
        let mut quads = frame.quad_iter(&resident)?.peekable();
        let count = frame.draws.len().div_ceil(VERTEX_CHUNK_QUADS);
        self.vertices
            .resize_with(count.max(self.vertices.len()), VertexChunk::default);
        for chunk in &mut self.vertices {
            chunk.bytes.clear();
            chunk.states.clear();
        }
        let mut uploaded = 0;
        let mut consumed = 0;
        let mut mapped = [0; 2];
        while let Some(quad) = quads.next() {
            for (i, at) in barriers.iter().enumerate() {
                if *at == consumed {
                    mapped[i] = uploaded;
                }
            }
            let quad = quad?;
            let mut second = 0.;
            let paired = if let Some(Ok(next)) = quads.peek() {
                if !barriers.contains(&(consumed + 1)) && additive_pair(&quad, next) {
                    second = next.vertices[0][4];
                    true
                } else {
                    false
                }
            } else {
                false
            };
            consumed += 1 + usize::from(paired);
            if paired {
                quads.next();
            }
            let chunk = &mut self.vertices[uploaded / VERTEX_CHUNK_QUADS];
            uploaded += 1;
            let texture = &self.textures[&quad.key];
            for vertex in [
                quad.vertices[0],
                quad.vertices[1],
                quad.vertices[2],
                quad.vertices[5],
            ] {
                for value in vertex {
                    chunk.bytes.extend_from_slice(&value.to_le_bytes());
                }
                chunk.bytes.extend_from_slice(&second.to_le_bytes());
                chunk.bytes.extend_from_slice(&texture.layer.to_le_bytes());
            }
            let mut state = DrawState::new(&quad, texture.black);
            state.key = texture.array.key;
            if paired {
                state.pipeline = Pipeline::PairLighter;
                state.noop = zero_pair(quad.vertices[0][4], second);
            }
            chunk.states.push(state);
        }
        for chunk in &mut self.vertices[..count] {
            if chunk.bytes.is_empty() {
                continue;
            }
            let capacity = (chunk.bytes.len() as u64).div_ceil(QUAD_VERTEX_BYTES as u64)
                * (QUAD_VERTEX_BYTES as u64);
            if chunk
                .buffer
                .as_ref()
                .is_none_or(|buffer| buffer.size() < capacity)
            {
                chunk.buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("surrounding ordered image chunk"),
                    size: capacity,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
            }
            self.queue
                .write_buffer(chunk.buffer.as_ref().expect("chunk"), 0, &chunk.bytes);
        }
        for (i, at) in barriers.iter().enumerate() {
            if *at == consumed {
                mapped[i] = uploaded;
            }
        }
        Ok(mapped)
    }
}
