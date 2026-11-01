//! Shared WebGPU/DX12/Vulkan texture-quad renderer. No sorting, game clock or Canvas fallback.
use crate::{
    Result,
    frame::{Frame, QUAD_VERTEX_BYTES, ResourceKey, VERTEX_CHUNK_QUADS},
    scene::Resource,
};
use slither_client::presentation::{BodyAsset, BodyAssetStatus};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[path = "draw_runs.rs"]
mod draw_runs;
use draw_runs::{DrawState, Pipeline, draw_runs};

#[derive(Debug, PartialEq, Eq)]
pub enum RenderError {
    Frame(String),
    Host(String),
}
impl From<String> for RenderError {
    fn from(error: String) -> Self {
        Self::Frame(error)
    }
}
impl From<&str> for RenderError {
    fn from(error: &str) -> Self {
        Self::Frame(error.into())
    }
}
impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Frame(e) | Self::Host(e) => f.write_str(e),
        }
    }
}
impl From<RenderError> for String {
    fn from(error: RenderError) -> Self {
        match error {
            RenderError::Frame(e) | RenderError::Host(e) => e,
        }
    }
}
type RenderResult<T> = std::result::Result<T, RenderError>;

struct Texture {
    asset: BodyAsset,
    _texture: wgpu::Texture,
    bind: wgpu::BindGroup,
    repeat_bind: wgpu::BindGroup,
    black: bool,
}
#[derive(Default)]
struct VertexChunk {
    buffer: Option<wgpu::Buffer>,
    bytes: Vec<u8>,
    states: Vec<DrawState>,
}
struct Multisample {
    extent: [u32; 2],
    samples: u32,
    view: wgpu::TextureView,
}
pub struct Renderer {
    pub sample_count: u32,
    pub(crate) format: wgpu::TextureFormat,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter_info: wgpu::AdapterInfo,
    layout: wgpu::BindGroupLayout,
    over: wgpu::RenderPipeline,
    overlay_over: wgpu::RenderPipeline,
    overlay_black_over: wgpu::RenderPipeline,
    overlay_lighter: wgpu::RenderPipeline,
    lighter: wgpu::RenderPipeline,
    black_over: wgpu::RenderPipeline,
    textures: BTreeMap<ResourceKey, Texture>,
    lost: Arc<AtomicBool>,
    multisample: Option<Multisample>,
    vertices: Vec<VertexChunk>,
    draw_calls: usize,
    diagnostic_timestamps: Option<wgpu::QuerySet>,
    #[cfg(feature = "frame-profile")]
    diagnostic_passes: Option<Vec<(&'static str, u32)>>,
}
impl Renderer {
    pub fn supports_native(&self, physical: [u32; 2]) -> bool {
        // Capability includes Native's single-sample path at large outputs.
        crate::world_resolution::validate_target(
            physical,
            1,
            self.device.limits().max_texture_dimension_2d,
        )
        .is_ok()
    }
    pub fn multisample_extent(&self) -> Option<([u32; 2], u32)> {
        self.multisample.as_ref().map(|m| (m.extent, m.samples))
    }
    pub fn validate_frame(&self, frame: &Frame) -> RenderResult<()> {
        frame
            .validate(
                &self
                    .textures
                    .iter()
                    .map(|(key, t)| (*key, t.asset))
                    .collect(),
            )
            .map_err(Into::into)
    }
    pub async fn new(
        instance: &wgpu::Instance,
        surface: Option<&wgpu::Surface<'_>>,
        format: wgpu::TextureFormat,
    ) -> Result<Self> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                force_fallback_adapter: false,
                compatible_surface: surface,
                ..Default::default()
            })
            .await
            .map_err(|e| format!("GPU unavailable: {e}"))?;
        Self::from_adapter(adapter, format).await
    }
    pub async fn from_adapter(adapter: wgpu::Adapter, format: wgpu::TextureFormat) -> Result<Self> {
        Self::from_adapter_with_max_samples(adapter, format, 4).await
    }
    /// Sample-one adapters and finite diagnostics use the same presentation path.
    pub async fn from_adapter_with_max_samples(
        adapter: wgpu::Adapter,
        format: wgpu::TextureFormat,
        max_samples: u32,
    ) -> Result<Self> {
        Self::from_adapter_with_features(adapter, format, max_samples, wgpu::Features::empty())
            .await
    }
    /// Explicit offscreen diagnostics only; ordinary hosts request no optional features.
    pub async fn from_adapter_with_features(
        adapter: wgpu::Adapter,
        format: wgpu::TextureFormat,
        max_samples: u32,
        required_features: wgpu::Features,
    ) -> Result<Self> {
        Self::from_adapter_options(
            adapter,
            format,
            max_samples,
            required_features,
            false,
            wgpu::Trace::Off,
        )
        .await
    }
    /// Finite offscreen diagnostics can opt into pass timestamps. Normal hosts
    /// request no additional device features and allocate no queries.
    pub async fn from_adapter_for_diagnostics(
        adapter: wgpu::Adapter,
        format: wgpu::TextureFormat,
        max_samples: u32,
        timestamps: bool,
    ) -> Result<Self> {
        let features = if timestamps {
            wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS
        } else {
            wgpu::Features::empty()
        };
        #[cfg(feature = "frame-profile")]
        let trace = if let Some(root) = std::env::var_os("FRAME_PERF_GPU_TRACE_DIR") {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let info = adapter.get_info();
            let path = std::path::PathBuf::from(root).join(format!(
                "{:04x}-{:04x}-{}",
                info.vendor,
                info.device,
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).map_err(|e| format!("GPU trace directory: {e}"))?;
            std::fs::write(
                path.join(".frame-perf-gpu-trace"),
                b"owned diagnostic trace\n",
            )
            .map_err(|e| e.to_string())?;
            wgpu::Trace::Directory(path)
        } else {
            wgpu::Trace::Off
        };
        #[cfg(not(feature = "frame-profile"))]
        let trace = wgpu::Trace::Off;
        Self::from_adapter_options(adapter, format, max_samples, features, timestamps, trace).await
    }
    async fn from_adapter_options(
        adapter: wgpu::Adapter,
        format: wgpu::TextureFormat,
        max_samples: u32,
        required_features: wgpu::Features,
        timestamps: bool,
        trace: wgpu::Trace,
    ) -> Result<Self> {
        if !matches!(max_samples, 1 | 4) {
            return Err("Unsupported sample count".into());
        }
        let info = adapter.get_info();
        let supported = adapter.get_texture_format_features(format).flags;
        let sample_count = if max_samples == 4
            && supported.contains(
                wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X4
                    | wgpu::TextureFormatFeatureFlags::MULTISAMPLE_RESOLVE,
            ) {
            4
        } else {
            1
        };
        if info.device_type == wgpu::DeviceType::Cpu {
            return Err("software/CPU adapter is not GPU success".into());
        }
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("synthetic-gameplay-slice"),
                required_features,
                trace,
                // Every backend validates against WebGL2 so native runs reject what
                // the planned WebGL2 extension cannot create. Surfaces and offscreen
                // targets keep the adapter's resolution; uploads have their own cap.
                required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await
            .map_err(|e| e.to_string())?;
        let lost = Arc::new(AtomicBool::new(false));
        let flag = lost.clone();
        device.set_device_lost_callback(move |_, _| {
            flag.store(true, Ordering::SeqCst);
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("source"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("paint"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("quad"),
            source: wgpu::ShaderSource::Wgsl(include_str!("quad.wgsl").into()),
        });
        let pipeline =
            |blend, fragment, samples| {
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("ordered paint"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: 20,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0=>Float32x2,1=>Float32x2,2=>Float32],
                    })],
                },
                primitive: Default::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState{count:samples,..Default::default()},
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fragment),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(blend),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
            };
        let over = pipeline(
            wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
            "fs",
            sample_count,
        );
        let overlay_over = pipeline(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING, "fs", 1);
        let black_blend = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::ReverseSubtract,
            },
            alpha: wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING.alpha,
        };
        let black_over = pipeline(black_blend, "fs_black", sample_count);
        let overlay_black_over = pipeline(black_blend, "fs_black", 1);
        let add = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        };
        let lighter = pipeline(
            wgpu::BlendState {
                color: add,
                alpha: add,
            },
            "fs",
            sample_count,
        );
        let overlay_lighter = pipeline(
            wgpu::BlendState {
                color: add,
                alpha: add,
            },
            "fs",
            1,
        );
        let diagnostic_timestamps = timestamps.then(|| {
            device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("offscreen pass timings"),
                ty: wgpu::QueryType::Timestamp,
                count: 8,
            })
        });
        Ok(Self {
            sample_count,
            format,
            device,
            queue,
            adapter_info: info,
            layout,
            over,
            overlay_over,
            overlay_black_over,
            overlay_lighter,
            lighter,
            black_over,
            textures: BTreeMap::new(),
            lost,
            multisample: None,
            vertices: Vec::new(),
            draw_calls: 0,
            diagnostic_timestamps,
            #[cfg(feature = "frame-profile")]
            diagnostic_passes: None,
        })
    }
    pub fn upload(&mut self, r: &Resource) -> RenderResult<()> {
        let a = r.asset;
        let key = ResourceKey::from(a);
        if self.lost.load(Ordering::SeqCst) {
            return Err(RenderError::Host(
                "GPU device lost; host stopped, scene retained".into(),
            ));
        }
        // WebGL2's 2048px downlevel cap keeps the future WebGL2 extension path valid.
        let max_dimension = wgpu::Limits::downlevel_webgl2_defaults().max_texture_dimension_2d;
        if a.status != BodyAssetStatus::Ready
            || a.width == 0
            || a.height == 0
            || a.width > max_dimension
            || a.height > max_dimension
            || r.image.width != a.width
            || r.image.height != a.height
            || r.image.pixels.len() != a.width as usize * a.height as usize * 4
        {
            return Err("invalid resource".into());
        }
        if self
            .textures
            .keys()
            .any(|k| k.handle == key.handle && k.revision >= key.revision)
        {
            return Err("duplicate/stale immutable resource revision".into());
        }
        if self.textures.len() >= 2048 && !self.textures.keys().any(|k| k.handle == a.handle) {
            return Err("texture count budget".into());
        }
        let retained = self
            .textures
            .values()
            .filter(|t| t.asset.handle != a.handle)
            .map(|t| t.asset.width as usize * t.asset.height as usize * 4)
            .sum::<usize>();
        if retained + r.image.pixels.len() > crate::bank::PIXEL_BUDGET {
            return Err("Resident GPU pixel budget".into());
        }
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("authored RGBA"),
            size: wgpu::Extent3d {
                width: a.width,
                height: a.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &r.image.pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(a.width * 4),
                rows_per_image: Some(a.height),
            },
            wgpu::Extent3d {
                width: a.width,
                height: a.height,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&Default::default());
        let binding = |address_mode| {
            let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("Canvas-like linear clamp"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                address_mode_u: address_mode,
                address_mode_v: address_mode,
                ..Default::default()
            });
            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("source"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            })
        };
        let bind = binding(wgpu::AddressMode::ClampToEdge);
        let repeat_bind = binding(wgpu::AddressMode::Repeat);
        // A new revision retires all prior versions of this handle atomically after validation/upload.
        self.textures.retain(|k, _| k.handle != key.handle);
        self.textures.insert(
            key,
            Texture {
                asset: a,
                _texture: texture,
                bind,
                repeat_bind,
                black: r
                    .image
                    .pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .all(|p| p[..3] == [0; 3]),
            },
        );
        Ok(())
    }
    pub fn release(&mut self, key: ResourceKey) -> bool {
        self.textures.remove(&key).is_some()
    }
    pub fn sync_resources(&mut self, resources: &[Resource]) -> RenderResult<()> {
        if self.lost.load(Ordering::SeqCst) {
            return Err(RenderError::Host(
                "GPU device lost; host stopped, scene retained".into(),
            ));
        }
        if resources.len() > 2048
            || resources
                .iter()
                .map(|r| r.image.pixels.len())
                .sum::<usize>()
                > crate::bank::PIXEL_BUDGET
        {
            return Err("Active GPU resource budget".into());
        }
        let wanted: std::collections::BTreeSet<_> = resources
            .iter()
            .map(|r| ResourceKey::from(r.asset))
            .collect();
        self.textures.retain(|key, _| wanted.contains(key));
        for r in resources {
            if let Some(t) = self.textures.get(&r.asset.into()) {
                if t.asset != r.asset {
                    return Err("Resident resource metadata changed".into());
                }
            } else {
                self.upload(r)?;
            }
        }
        Ok(())
    }
    pub fn render(&mut self, frame: &Frame, target: &wgpu::TextureView) -> RenderResult<()> {
        self.render_world_at(frame, target, frame.extent, self.sample_count)
    }
    /// Change attachment pixels only; Frame::quad_iter still uses the original projection.
    pub fn render_world_at(
        &mut self,
        frame: &Frame,
        target: &wgpu::TextureView,
        attachment: [u32; 2],
        samples: u32,
    ) -> RenderResult<()> {
        if samples != 1 && samples != self.sample_count {
            return Err("Unsupported world sample count".into());
        }
        self.render_inner(frame, target, false, attachment, samples)
    }
    /// Adjacent draw runs in prepared vertex chunks; read after a successful render.
    pub fn draw_call_count(&self) -> usize {
        self.draw_calls
    }
    /// The last render pass's start/end queries, before the next render call.
    /// Resolve/readback belongs to the diagnostic caller, outside CPU timings.
    pub fn diagnostic_timestamps(&self) -> Option<&wgpu::QuerySet> {
        self.diagnostic_timestamps.as_ref()
    }
    /// Start a multi-pass diagnostic picture; timing callers resolve after the
    /// complete presenter. Legacy single-pass callers still use queries 0/1.
    #[cfg(feature = "frame-profile")]
    pub fn begin_diagnostic_frame(&mut self) {
        self.diagnostic_passes = Some(Vec::with_capacity(3));
    }
    #[cfg(feature = "frame-profile")]
    pub fn diagnostic_passes(&self) -> &[(&'static str, u32)] {
        self.diagnostic_passes.as_deref().unwrap_or(&[])
    }
    #[cfg(feature = "frame-profile")]
    fn diagnostic_pass(&mut self, name: &'static str) -> RenderResult<(u32, u32)> {
        let Some(passes) = &mut self.diagnostic_passes else {
            return Ok((0, 1));
        };
        let start = passes.len() as u32 * 2;
        if start + 1 >= 8 {
            return Err("diagnostic pass query limit".into());
        }
        passes.push((name, start));
        Ok((start, start + 1))
    }
    #[cfg(feature = "frame-profile")]
    pub fn begin_diagnostic_encoder_pass(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        name: &'static str,
    ) -> RenderResult<Option<u32>> {
        if self.diagnostic_passes.is_none() || self.diagnostic_timestamps.is_none() {
            return Ok(None);
        }
        let (start, end) = self.diagnostic_pass(name)?;
        encoder.write_timestamp(
            self.diagnostic_timestamps
                .as_ref()
                .expect("diagnostic queries"),
            start,
        );
        Ok(Some(end))
    }
    #[cfg(feature = "frame-profile")]
    pub fn end_diagnostic_encoder_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        end: Option<u32>,
    ) {
        if let Some(end) = end {
            encoder.write_timestamp(
                self.diagnostic_timestamps
                    .as_ref()
                    .expect("diagnostic queries"),
                end,
            );
        }
    }
    /// Delimit actual wgpu API-trace input frames. Used only in a separate
    /// validation run, never in CPU/GPU timing captures.
    #[cfg(feature = "frame-profile")]
    pub fn diagnostic_trace_marker(&self, label: &str) {
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.insert_debug_marker(label);
        self.queue.submit([encoder.finish()]);
    }
    /// DOM siblings paint over the blit with the same quads, without a window-sized MSAA target.
    pub fn render_overlay(
        &mut self,
        frame: &Frame,
        target: &wgpu::TextureView,
    ) -> RenderResult<()> {
        self.render_inner(frame, target, true, frame.extent, 1)
    }
    fn render_inner(
        &mut self,
        frame: &Frame,
        target: &wgpu::TextureView,
        overlay: bool,
        attachment: [u32; 2],
        samples: u32,
    ) -> RenderResult<()> {
        if self.lost.load(Ordering::SeqCst) {
            return Err(RenderError::Host(
                "GPU device lost; no gameplay reset/retry".into(),
            ));
        }
        let resident = self
            .textures
            .iter()
            .map(|(key, t)| (*key, t.asset))
            .collect();
        let quads = frame.quad_iter(&resident)?;
        if frame.extent.contains(&0) {
            return Ok(());
        }
        crate::world_resolution::validate_target(
            attachment,
            samples,
            if overlay {
                // Preserve the existing physical surface overlay guard. Native
                // world allocation separately checks negotiated texture limits.
                crate::frame::MAX_EXTENT
            } else {
                self.device.limits().max_texture_dimension_2d
            },
        )?;
        let chunk_count = frame.draws.len().div_ceil(VERTEX_CHUNK_QUADS);
        self.vertices
            .resize_with(chunk_count.max(self.vertices.len()), VertexChunk::default);
        for chunk in &mut self.vertices {
            chunk.bytes.clear();
            chunk.states.clear();
        }
        for (index, quad) in quads.enumerate() {
            let quad = quad?;
            let chunk = &mut self.vertices[index / VERTEX_CHUNK_QUADS];
            if chunk.states.is_empty() {
                let count = (frame.draws.len() - index).min(VERTEX_CHUNK_QUADS);
                if chunk.bytes.capacity() < count * QUAD_VERTEX_BYTES {
                    chunk.bytes.reserve_exact(count * QUAD_VERTEX_BYTES);
                }
                if chunk.states.capacity() < count {
                    chunk.states.reserve_exact(count);
                }
            }
            for vertex in quad.vertices {
                for value in vertex {
                    chunk.bytes.extend_from_slice(&value.to_le_bytes());
                }
            }
            chunk
                .states
                .push(DrawState::new(&quad, self.textures[&quad.key].black));
        }
        // Validate every quad before touching GPU buffers. Exclusive calls submit
        // before the next write; queue ordering protects prior in-flight draws,
        // including when the overlay reuses the world's buffers.
        for chunk in &mut self.vertices[..chunk_count] {
            let size = chunk.bytes.len() as u64;
            if chunk.buffer.as_ref().is_none_or(|b| b.size() < size) {
                let capacity = size
                    .next_power_of_two()
                    .min((VERTEX_CHUNK_QUADS * QUAD_VERTEX_BYTES) as u64);
                chunk.buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("ordered vertex chunk"),
                    size: capacity,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
            }
            self.queue.write_buffer(
                chunk.buffer.as_ref().expect("vertex buffer"),
                0,
                &chunk.bytes,
            );
        }
        let replacement = if samples > 1
            && self
                .multisample
                .as_ref()
                .is_none_or(|m| m.extent != attachment || m.samples != samples)
        {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("quad edge coverage"),
                size: wgpu::Extent3d {
                    width: attachment[0],
                    height: attachment[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format: self.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            Some(Multisample {
                extent: attachment,
                samples,
                view: texture.create_view(&Default::default()),
            })
        } else {
            None
        };
        #[cfg(feature = "frame-profile")]
        let query_indices = self.diagnostic_pass(if overlay { "overlay" } else { "world" })?;
        #[cfg(not(feature = "frame-profile"))]
        let query_indices = (0, 1);
        let msaa_view = (samples > 1).then(|| {
            &replacement
                .as_ref()
                .or(self.multisample.as_ref())
                .expect("MSAA target")
                .view
        });
        let mut draw_calls = 0;
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("offline gameplay"),
                timestamp_writes: self.diagnostic_timestamps.as_ref().map(|query_set| {
                    wgpu::RenderPassTimestampWrites {
                        query_set,
                        beginning_of_pass_write_index: Some(query_indices.0),
                        end_of_pass_write_index: Some(query_indices.1),
                    }
                }),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: msaa_view.unwrap_or(target),
                    depth_slice: None,
                    resolve_target: msaa_view.map(|_| target),
                    ops: wgpu::Operations {
                        load: if overlay {
                            wgpu::LoadOp::Load
                        } else {
                            wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.035,
                                g: 0.045,
                                b: 0.06,
                                a: 1.,
                            })
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            let mut previous: Option<DrawState> = None;
            for chunk in &self.vertices[..chunk_count] {
                pass.set_vertex_buffer(0, chunk.buffer.as_ref().expect("vertex buffer").slice(..));
                for (state, vertices) in draw_runs(chunk.states.iter().copied()) {
                    if previous.is_none_or(|p| p.pipeline != state.pipeline) {
                        pass.set_pipeline(match state.pipeline {
                            Pipeline::BlackOver => {
                                if overlay || samples != self.sample_count {
                                    &self.overlay_black_over
                                } else {
                                    &self.black_over
                                }
                            }
                            Pipeline::Lighter => {
                                if overlay || samples != self.sample_count {
                                    &self.overlay_lighter
                                } else {
                                    &self.lighter
                                }
                            }
                            Pipeline::Over => {
                                if overlay || samples != self.sample_count {
                                    &self.overlay_over
                                } else {
                                    &self.over
                                }
                            }
                        });
                    }
                    if previous.is_none_or(|p| (p.key, p.repeat) != (state.key, state.repeat)) {
                        let texture = &self.textures[&state.key];
                        pass.set_bind_group(
                            0,
                            if state.repeat {
                                &texture.repeat_bind
                            } else {
                                &texture.bind
                            },
                            &[],
                        );
                    }
                    pass.draw(vertices, 0..1);
                    draw_calls += 1;
                    previous = Some(state);
                }
            }
        }
        let submission = crate::latency::submit_begin();
        self.queue.submit([encoder.finish()]);
        crate::latency::submit_end(submission);
        self.draw_calls = draw_calls;
        // Allocation precedes publication; wgpu retains submitted attachment references
        // after the old host handles are dropped, without a CPU/GPU idle wait.
        if let Some(replacement) = replacement {
            self.multisample = Some(replacement);
        }
        if !overlay && samples == 1 {
            self.multisample = None;
        }
        Ok(())
    }
}

pub fn instance() -> wgpu::Instance {
    wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: if cfg!(target_arch = "wasm32") {
            wgpu::Backends::BROWSER_WEBGPU
        } else {
            wgpu::Backends::DX12 | wgpu::Backends::VULKAN
        },
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    })
}

#[derive(Debug, PartialEq, Eq)]
enum SurfaceRecovery {
    Retry,
    WaitUntilVisible,
    Reconfigure,
}
fn surface_recovery(failure: wgpu::CurrentSurfaceTexture) -> Result<SurfaceRecovery> {
    match failure {
        wgpu::CurrentSurfaceTexture::Timeout => Ok(SurfaceRecovery::Retry),
        wgpu::CurrentSurfaceTexture::Occluded => Ok(SurfaceRecovery::WaitUntilVisible),
        wgpu::CurrentSurfaceTexture::Outdated => Ok(SurfaceRecovery::Reconfigure),
        // Lost invalidates the surface itself, not just its configuration. Let
        // the host shut down in order rather than configure an invalid surface.
        // https://docs.rs/wgpu/30.0.1/wgpu/enum.CurrentSurfaceTexture.html#variant.Lost
        wgpu::CurrentSurfaceTexture::Lost => Err(
            "surface lost; host stopped, scene retained; explicit surface recreation required"
                .into(),
        ),
        e => Err(format!("surface stopped: {e:?}")),
    }
}

/// Windows and browser surface owner. Time/scene are outside this owner.
pub struct SurfaceHost {
    pub renderer: Renderer,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    extent: [u32; 2],
    world: crate::world_presenter::WorldPresenter,
    acquired: Option<wgpu::SurfaceTexture>,
    occluded: bool,
}
impl SurfaceHost {
    pub async fn new(
        instance: &wgpu::Instance,
        surface: wgpu::Surface<'static>,
        extent: [u32; 2],
    ) -> Result<Self> {
        // Choose an encoded-unorm target to keep the declared encoded-sRGB blend domain.
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .map_err(|e| e.to_string())?;
        let caps = surface.get_capabilities(&adapter);
        let format = *caps
            .formats
            .iter()
            .find(|f| {
                matches!(
                    f,
                    wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm
                )
            })
            .ok_or("encoded unorm surface unsupported")?;
        if !caps
            .color_spaces(format)
            .contains(wgpu::SurfaceColorSpaces::SRGB)
            || !caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque)
        {
            return Err("opaque sRGB presentation surface unsupported".into());
        }
        let renderer = Renderer::from_adapter(adapter, format).await?;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Srgb,
            width: extent[0].max(1),
            height: extent[1].max(1),
            present_mode: wgpu::PresentMode::Fifo,
            // Native prioritizes latency over CPU/GPU overlap; this is a backend
            // hint, not a measured Chrome comparison. Keep the browser's hint.
            // https://docs.rs/wgpu/30.0.1/wgpu/type.SurfaceConfiguration.html#structfield.desired_maximum_frame_latency
            desired_maximum_frame_latency: if cfg!(target_arch = "wasm32") { 2 } else { 1 },
            alpha_mode: wgpu::CompositeAlphaMode::Opaque,
            view_formats: vec![],
        };
        let world = crate::world_presenter::WorldPresenter::new(&renderer);
        let mut host = Self {
            renderer,
            surface,
            config,
            extent: [0, 0],
            world,
            acquired: None,
            occluded: false,
        };
        host.resize(extent)?;
        Ok(host)
    }
    pub fn resize(&mut self, extent: [u32; 2]) -> Result<()> {
        if extent.iter().any(|n| *n > crate::frame::MAX_EXTENT) {
            return Err("surface extent limit".into());
        }
        if self.extent == extent {
            return Ok(());
        }
        self.extent = extent;
        self.acquired = None;
        if !extent.contains(&0) {
            self.config.width = extent[0];
            self.config.height = extent[1];
            self.surface.configure(&self.renderer.device, &self.config);
        }
        Ok(())
    }
    pub fn present(&mut self, frame: &Frame) -> Result<bool> {
        self.present_inner(
            frame,
            false,
            None,
            slither_client::settings::WorldResolution::Original,
        )
        .map_err(Into::into)
    }
    /// Render at the desktop policy size, then scale the complete image like a CSS canvas.
    pub fn present_scaled(&mut self, frame: &Frame) -> Result<bool> {
        self.present_inner(
            frame,
            true,
            None,
            slither_client::settings::WorldResolution::Original,
        )
        .map_err(Into::into)
    }
    pub fn present_scaled_with_overlay(
        &mut self,
        frame: &Frame,
        overlay: Option<&Frame>,
    ) -> RenderResult<bool> {
        self.present_inner(
            frame,
            true,
            overlay,
            slither_client::settings::WorldResolution::Original,
        )
    }
    pub fn present_world_with_overlay(
        &mut self,
        frame: &Frame,
        overlay: Option<&Frame>,
        requested: slither_client::settings::WorldResolution,
    ) -> RenderResult<bool> {
        self.present_inner(frame, true, overlay, requested)
    }
    /// Native waits for FIFO capacity before sampling the next model timestamp
    /// and draining ingress, rather than staging an update ahead of that wait.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn prepare_present(&mut self) -> Result<bool> {
        self.acquire()
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn is_occluded(&self) -> bool {
        self.occluded
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn discard_acquired(&mut self) {
        self.acquired = None;
    }
    fn acquire(&mut self) -> Result<bool> {
        if self.extent.contains(&0) {
            return Ok(false);
        }
        if self.acquired.is_some() {
            return Ok(true);
        }
        self.occluded = false;
        let acquire = crate::latency::acquire_begin();
        let texture = self.surface.get_current_texture();
        crate::latency::event("acquire_end", acquire, 0, 0);
        self.acquired = Some(match texture {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            failure => {
                match surface_recovery(failure)? {
                    SurfaceRecovery::Retry => {}
                    SurfaceRecovery::WaitUntilVisible => self.occluded = true,
                    SurfaceRecovery::Reconfigure => {
                        self.surface.configure(&self.renderer.device, &self.config);
                    }
                }
                return Ok(false);
            }
        });
        Ok(true)
    }
    #[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
    pub fn present_with_menu(
        &mut self,
        frame: &Frame,
        overlay: Option<&Frame>,
        requested: slither_client::settings::WorldResolution,
        menus: &mut crate::egui_native::NativeEguiHost,
    ) -> RenderResult<bool> {
        self.present_inner_with(frame, true, overlay, requested, |renderer, view| {
            menus.paint(renderer, view)
        })
    }
    fn present_inner(
        &mut self,
        frame: &Frame,
        scale: bool,
        overlay: Option<&Frame>,
        requested: slither_client::settings::WorldResolution,
    ) -> RenderResult<bool> {
        self.present_inner_with(frame, scale, overlay, requested, |_, _| {})
    }
    fn present_inner_with(
        &mut self,
        frame: &Frame,
        scale: bool,
        overlay: Option<&Frame>,
        requested: slither_client::settings::WorldResolution,
        paint: impl FnOnce(&Renderer, &wgpu::TextureView),
    ) -> RenderResult<bool> {
        if overlay.is_some_and(|f| f.extent != self.extent) {
            return Err("overlay/surface extent mismatch".into());
        }
        let scaled = frame.extent != self.extent;
        if scaled && !scale {
            return Err("frame/surface extent mismatch".into());
        }
        if self.extent.contains(&0) || frame.extent.contains(&0) {
            return Ok(false);
        }
        crate::world_resolution::WorldTargetPlan::new(
            requested,
            frame.extent,
            self.extent,
            self.renderer.sample_count,
            self.renderer.device.limits().max_texture_dimension_2d,
        )?;
        if !self.acquire().map_err(RenderError::Host)? {
            return Ok(false);
        }
        let texture = self.acquired.take().expect("acquired surface");
        let view = texture.texture.create_view(&Default::default());
        self.world.render(
            &mut self.renderer,
            frame,
            overlay,
            &view,
            self.extent,
            requested,
        )?;
        paint(&self.renderer, &view);
        crate::latency::picture("present_begin");
        self.renderer.queue.present(texture);
        crate::latency::picture("present_end");
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_and_record_storage_fit_webgl2_device_limits() {
        let limits = wgpu::Limits::downlevel_webgl2_defaults();
        assert_eq!(std::mem::size_of::<crate::frame::Draw>(), 256);
        assert_eq!(std::mem::size_of::<DrawState>(), 24);
        assert_eq!(crate::frame::MAX_DRAWS, 262_144);
        assert_eq!(VERTEX_CHUNK_QUADS * QUAD_VERTEX_BYTES, 983_040);
        assert!((VERTEX_CHUNK_QUADS * QUAD_VERTEX_BYTES) as u64 <= limits.max_buffer_size);
        assert!(20 <= limits.max_vertex_buffer_array_stride);
        assert!(3 <= limits.max_vertex_attributes);
        assert!(1 <= limits.max_vertex_buffers);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    #[ignore = "explicit finite offscreen GPU check"]
    fn retained_targets_and_chunks_preserve_inflight_world_and_overlay() {
        use slither_client::presentation::{BodyBlend, BodyRect};
        let mut r = pollster::block_on(Renderer::new(
            &instance(),
            None,
            wgpu::TextureFormat::Rgba8Unorm,
        ))
        .unwrap();
        let scope = r.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let mut assets = Vec::new();
        for (handle, pixels) in [(1, vec![255, 0, 0, 255]), (2, vec![0, 255, 0, 255])] {
            let asset = BodyAsset {
                handle,
                revision: 1,
                width: 1,
                height: 1,
                status: BodyAssetStatus::Ready,
            };
            r.upload(&Resource {
                asset,
                image: crate::materials::RgbaImage {
                    width: 1,
                    height: 1,
                    pixels,
                }
                .into(),
            })
            .unwrap();
            assets.push(asset);
        }
        let target = |extent: [u32; 2]| {
            r.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("inflight control"),
                size: wgpu::Extent3d {
                    width: extent[0],
                    height: extent[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            })
        };
        let a = target([64, 64]);
        let b = target([64, 64]);
        let resized = target([32, 64]);
        let mut frame = Frame {
            identity: 1,
            tick: 1,
            extent: [64, 64],
            viewport_scale: 1.,
            snakes: Vec::new(),
            draws: vec![crate::frame::Draw {
                corners: None,
                asset: assets[0],
                source: None,
                repeat_uv: None,
                black_effect: false,
                destination: BodyRect {
                    x: 0.,
                    y: 0.,
                    width: 64.,
                    height: 64.,
                },
                translation: [0.; 2],
                rotation: 0.,
                effective_alpha: 1.,
                blend: BodyBlend::SourceOver,
            }],
        };
        r.render(&frame, &a.create_view(&Default::default()))
            .unwrap();
        let small = r.vertices[0].buffer.clone().unwrap();
        let mut degenerate = frame.draws[0].clone();
        degenerate.destination.width = 0.;
        frame
            .draws
            .extend(std::iter::repeat_n(degenerate, VERTEX_CHUNK_QUADS));
        r.render(&frame, &a.create_view(&Default::default()))
            .unwrap();
        assert_ne!(small, *r.vertices[0].buffer.as_ref().unwrap());
        let buffers: Vec<_> = r.vertices.iter().map(|c| c.buffer.clone()).collect();
        let msaa = r.multisample.as_ref().map(|m| m.view.clone());
        frame.draws.truncate(1);
        frame.draws[0].asset = assets[1];
        frame.draws[0].destination.x = 32.;
        frame.draws[0].destination.width = 32.;
        r.render_overlay(&frame, &a.create_view(&Default::default()))
            .unwrap();
        frame.draws[0].destination.x = 0.;
        frame.draws[0].destination.width = 64.;
        r.render(&frame, &b.create_view(&Default::default()))
            .unwrap();
        assert_eq!(
            buffers,
            r.vertices
                .iter()
                .map(|c| c.buffer.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(msaa, r.multisample.as_ref().map(|m| m.view.clone()));
        frame.extent = [32, 64];
        r.render(&frame, &resized.create_view(&Default::default()))
            .unwrap();
        if r.sample_count > 1 {
            assert_ne!(msaa, r.multisample.as_ref().map(|m| m.view.clone()));
        }
        // No wait between the writes/submissions above, including growth and resize.
        for (texture, left) in [(&a, [255, 0, 0, 255]), (&b, [0, 255, 0, 255])] {
            let buffer = r.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: 64 * 64 * 4,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let mut encoder = r.device.create_command_encoder(&Default::default());
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(256),
                        rows_per_image: Some(64),
                    },
                },
                texture.size(),
            );
            r.queue.submit([encoder.finish()]);
            let (tx, rx) = std::sync::mpsc::channel();
            buffer.slice(..).map_async(wgpu::MapMode::Read, move |v| {
                tx.send(v).unwrap();
            });
            r.device
                .poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: Some(std::time::Duration::from_secs(10)),
                })
                .unwrap();
            rx.recv().unwrap().unwrap();
            let data = buffer.slice(..).get_mapped_range().unwrap();
            assert_eq!(&data[4 * (32 * 64 + 16)..][..4], &left);
            assert_eq!(&data[4 * (32 * 64 + 48)..][..4], &[0, 255, 0, 255]);
            drop(data);
            buffer.unmap();
        }
        assert!(pollster::block_on(scope.pop()).is_none());
        r.lost.store(true, Ordering::SeqCst);
        assert!(matches!(r.sync_resources(&[]), Err(RenderError::Host(_))));
        assert!(matches!(
            r.render(&frame, &resized.create_view(&Default::default())),
            Err(RenderError::Host(_))
        ));
    }

    #[test]
    fn surface_recovery_fails_on_lost_and_only_reconfigures_outdated() {
        assert_eq!(
            surface_recovery(wgpu::CurrentSurfaceTexture::Lost),
            Err(
                "surface lost; host stopped, scene retained; explicit surface recreation required"
                    .into()
            ),
        );
        assert_eq!(
            surface_recovery(wgpu::CurrentSurfaceTexture::Outdated),
            Ok(SurfaceRecovery::Reconfigure)
        );
        assert_eq!(
            surface_recovery(wgpu::CurrentSurfaceTexture::Timeout),
            Ok(SurfaceRecovery::Retry)
        );
        assert_eq!(
            surface_recovery(wgpu::CurrentSurfaceTexture::Occluded),
            Ok(SurfaceRecovery::WaitUntilVisible)
        );
        assert!(surface_recovery(wgpu::CurrentSurfaceTexture::Validation).is_err());
    }
}
