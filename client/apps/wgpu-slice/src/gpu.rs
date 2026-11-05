//! Shared WebGPU/DX12/Vulkan texture-quad renderer; no game clock or Canvas fallback.
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
#[path = "texture_arrays.rs"]
mod texture_arrays;
use texture_arrays::{Group, TextureArray};
#[path = "pellet_cpu.rs"]
mod pellet_cpu;
#[path = "pellet_gpu.rs"]
mod pellet_gpu;
#[path = "soft_layers.rs"]
mod soft_layers;
use soft_layers::{LayerHistory, covered_area, pays, select_glows};

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

fn additive_pair(a: &crate::frame::Quad, b: &crate::frame::Quad) -> bool {
    a.blend == slither_client::presentation::BodyBlend::Lighter
        && b.blend == a.blend
        && b.key == a.key
        && b.repeat == a.repeat
        && b.black_effect == a.black_effect
        && a.vertices
            .iter()
            .zip(b.vertices.iter())
            .all(|(a, b)| a[..4] == b[..4])
}

fn glow_bin(quad: &crate::frame::Quad, second_alpha: f32, attachment: [u32; 2]) -> u8 {
    let a = quad.vertices[0];
    let b = quad.vertices[1];
    let c = quad.vertices[5];
    let edge = |v: [f32; 5]| {
        ((f64::from(v[0] - a[0]) * f64::from(attachment[0])).powi(2)
            + (f64::from(v[1] - a[1]) * f64::from(attachment[1])).powi(2))
        .sqrt()
            / 2.
    };
    let side = edge(b).min(edge(c));
    // Preserve the existing size/opacity guard for every reduced attachment.
    let opacity = quad.vertices[0][4].max(second_alpha);
    [(3, 8.), (2, 4.), (1, 2.)]
        .into_iter()
        .find(|&(_, factor)| side >= 4. * factor && f64::from(opacity) * factor / side <= 0.004)
        .map_or(0, |(scale, _)| scale)
}

fn zero_pair(a: f32, b: f32) -> bool {
    // Each sampled UNORM component is <= 1. fs_pair rounds each contribution
    // separately to a byte, including alpha. Leave a margin below the half-byte
    // boundary for interpolation/filter precision; never apply to ordinary fs.
    f64::from(a.max(b)) * 255. < 0.49
}

#[cfg(test)]
#[test]
fn zero_pairs_leave_a_margin_at_the_byte_rounding_boundary() {
    assert!(zero_pair(0., 0.48 / 255.));
    assert!(zero_pair(0.48 / 255., 0.));
    for alpha in [0.5 / 255., 0.51 / 255., 1.] {
        assert!(!zero_pair(alpha, 0.));
        assert!(!zero_pair(0., alpha));
    }
}
struct Texture {
    asset: BodyAsset,
    array: TextureArray,
    layer: u32,
    black: bool,
    food_glow: bool,
    food_outline: bool,
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

struct LayerTarget {
    extent: [u32; 2],
    view: wgpu::TextureView,
    bind: wgpu::BindGroup,
}
impl LayerTarget {
    fn new(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
        extent: [u32; 2],
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("single-sample food layer"),
            size: wgpu::Extent3d {
                width: extent[0],
                height: extent[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let sample_view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("food layer reconstruction"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&sample_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        Self { extent, view, bind }
    }
}
struct PaintPass<'a> {
    target: &'a wgpu::TextureView,
    resolve: Option<&'a wgpu::TextureView>,
    samples: u32,
    load: wgpu::LoadOp<wgpu::Color>,
    range: std::ops::Range<usize>,
    before: Option<std::ops::Range<usize>>,
    queries: Option<(u32, u32)>,
    coarse: Option<&'a wgpu::BindGroup>,
    fine: [Option<&'a wgpu::BindGroup>; 2],
    small_glows: Option<std::ops::Range<usize>>,
    filter: Option<u8>,
    base: Option<&'a wgpu::BindGroup>,
}
pub struct Renderer {
    /// Historical Auto ceiling, distinct from the last presented attachment.
    pub sample_count: u32,
    pub supported_samples: Vec<u32>,
    pub anti_aliasing: slither_client::settings::AntiAliasing,
    pub effective_sample_count: u32,
    pub(crate) format: wgpu::TextureFormat,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter_info: wgpu::AdapterInfo,
    layout: wgpu::BindGroupLayout,
    over: BTreeMap<u32, wgpu::RenderPipeline>,
    overlay_over: wgpu::RenderPipeline,
    overlay_black_over: wgpu::RenderPipeline,
    overlay_lighter: wgpu::RenderPipeline,
    lighter: BTreeMap<u32, wgpu::RenderPipeline>,
    pair_lighter: BTreeMap<u32, wgpu::RenderPipeline>,
    overlay_pair_lighter: wgpu::RenderPipeline,
    glow_lighter: [wgpu::RenderPipeline; 3],
    glow_pair_lighter: [wgpu::RenderPipeline; 3],
    black_over: BTreeMap<u32, wgpu::RenderPipeline>,
    textures: BTreeMap<ResourceKey, Texture>,
    arrays: BTreeMap<(u32, u32, u32, usize), Group>,
    bindings: BTreeMap<ResourceKey, TextureArray>,
    next_array: u64,
    indices: wgpu::Buffer,
    lost: Arc<AtomicBool>,
    multisample: Option<Multisample>,
    vertices: Vec<VertexChunk>,
    pellets: Option<pellet_gpu::Pellets>,
    glow_sort: Vec<(DrawState, [u8; QUAD_VERTEX_BYTES])>,
    glow: Option<LayerTarget>,
    fine_glow: Option<LayerTarget>,
    half_glow: Option<LayerTarget>,
    layer_history: LayerHistory,
    fine_composite_single: wgpu::RenderPipeline,
    outline_target: Option<LayerTarget>,
    outline_composite: BTreeMap<u32, wgpu::RenderPipeline>,
    glow_composite: BTreeMap<u32, wgpu::RenderPipeline>,
    glow_composite_single: wgpu::RenderPipeline,
    draw_calls: usize,
    pass_count: usize,
    diagnostic_timestamps: Option<wgpu::QuerySet>,
    #[cfg(feature = "frame-profile")]
    diagnostic_passes: Option<Vec<(&'static str, u32)>>,
}
impl Renderer {
    pub fn world_plan(
        &self,
        requested: slither_client::settings::WorldResolution,
        projection: [u32; 2],
        physical: [u32; 2],
    ) -> Result<crate::world_resolution::WorldTargetPlan> {
        crate::world_resolution::WorldTargetPlan::with_anti_aliasing(
            requested,
            projection,
            physical,
            self.sample_count,
            self.device.limits().max_texture_dimension_2d,
            self.anti_aliasing,
            &self.supported_samples,
        )
        .map_err(Into::into)
    }
    pub fn supports_native(&self, physical: [u32; 2]) -> bool {
        // Capability includes Native's single-sample path at large outputs.
        crate::world_resolution::validate_target(
            physical,
            1,
            self.device.limits().max_texture_dimension_2d,
        )
        .is_ok()
    }
    pub fn food_outline_extent(&self) -> Option<[u32; 2]> {
        self.outline_target.as_ref().map(|g| g.extent)
    }
    pub fn fine_glow_extent(&self) -> Option<[u32; 2]> {
        self.fine_glow.as_ref().map(|g| g.extent)
    }
    pub fn half_glow_extent(&self) -> Option<[u32; 2]> {
        self.half_glow.as_ref().map(|g| g.extent)
    }
    pub fn food_glow_extent(&self) -> Option<[u32; 2]> {
        self.glow.as_ref().map(|g| g.extent)
    }
    pub fn world_pass_count(&self) -> usize {
        self.pass_count
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
        if !matches!(max_samples, 1 | 2 | 4) {
            return Err("Unsupported sample count".into());
        }
        let info = adapter.get_info();
        // 2× is adapter-specific; Auto retains the historical 4/1 ceiling.
        let format_features = wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES;
        let adapter_formats = adapter.features().contains(format_features);
        let required_features = required_features
            | if adapter_formats {
                format_features
            } else {
                wgpu::Features::empty()
            };
        let supported = adapter.get_texture_format_features(format).flags;
        let supported_samples: Vec<_> = [1, 2, 4]
            .into_iter()
            .filter(|n| {
                *n <= max_samples
                    && (*n == 1
                        || (supported.sample_count_supported(*n)
                            && supported
                                .contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_RESOLVE)
                            && (*n != 2 || adapter_formats)))
            })
            .collect();
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
                        view_dimension: wgpu::TextureViewDimension::D2Array,
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
        let attributes = wgpu::vertex_attr_array![0=>Float32x2,1=>Float32x2,2=>Float32x2,3=>Uint32];
        let vertex_buffers = [Some(wgpu::VertexBufferLayout {
            array_stride: 28,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &attributes,
        })];
        let pipeline = |blend, fragment, samples, composite| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("ordered paint"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(if composite { "vs_composite" } else { "vs" }),
                    compilation_options: Default::default(),
                    buffers: if composite { &[] } else { &vertex_buffers },
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
                        blend: Some(blend),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let world_pipeline = |blend, fragment, composite| {
            supported_samples
                .iter()
                .map(|&samples| (samples, pipeline(blend, fragment, samples, composite)))
                .collect::<BTreeMap<_, _>>()
        };
        let over = world_pipeline(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING, "fs", false);
        let overlay_over = pipeline(
            wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
            "fs",
            1,
            false,
        );
        let black_blend = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::ReverseSubtract,
            },
            alpha: wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING.alpha,
        };
        let black_over = world_pipeline(black_blend, "fs_black", false);
        let overlay_black_over = pipeline(black_blend, "fs_black", 1, false);
        let add = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        };
        let lighter = world_pipeline(
            wgpu::BlendState {
                color: add,
                alpha: add,
            },
            "fs",
            false,
        );
        let overlay_lighter = pipeline(
            wgpu::BlendState {
                color: add,
                alpha: add,
            },
            "fs",
            1,
            false,
        );
        let glow_blend = wgpu::BlendState {
            color: add,
            alpha: add,
        };
        let pair_lighter = world_pipeline(glow_blend, "fs_pair", false);
        let overlay_pair_lighter = pipeline(glow_blend, "fs_pair", 1, false);
        let glow_lighter =
            ["fs_glow2", "fs_glow4", "fs_glow8"].map(|entry| pipeline(glow_blend, entry, 1, false));
        let glow_pair_lighter =
            ["fs_pair2", "fs_pair4", "fs_pair8"].map(|entry| pipeline(glow_blend, entry, 1, false));
        let outline_composite = world_pipeline(glow_blend, "fs", true);
        let fine_composite_single = pipeline(glow_blend, "fs", 1, true);
        let glow_composite = world_pipeline(glow_blend, "fs_glow_composite", true);
        let glow_composite_single = pipeline(glow_blend, "fs_glow_composite", 1, true);
        let diagnostic_timestamps = timestamps.then(|| {
            device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("offscreen pass timings"),
                ty: wgpu::QueryType::Timestamp,
                count: 32,
            })
        });
        let mut index_bytes = Vec::with_capacity(VERTEX_CHUNK_QUADS * 6 * 2);
        for quad in 0..VERTEX_CHUNK_QUADS as u16 {
            for index in [0, 1, 2, 0, 2, 3] {
                index_bytes.extend_from_slice(&(quad * 4 + index).to_le_bytes());
            }
        }
        let indices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shared quad indices"),
            size: index_bytes.len() as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&indices, 0, &index_bytes);
        let pellets = Some(pellet_gpu::Pellets::new(
            &device,
            &layout,
            format,
            &supported_samples,
        ));
        Ok(Self {
            sample_count,
            supported_samples,
            anti_aliasing: Default::default(),
            effective_sample_count: sample_count,
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
            pair_lighter,
            overlay_pair_lighter,
            glow_lighter,
            glow_pair_lighter,
            black_over,
            textures: BTreeMap::new(),
            arrays: BTreeMap::new(),
            bindings: BTreeMap::new(),
            next_array: 0,
            indices,
            lost,
            multisample: None,
            vertices: Vec::new(),
            pellets,
            glow_sort: Vec::new(),
            glow: None,
            fine_glow: None,
            half_glow: None,
            layer_history: LayerHistory::default(),
            fine_composite_single,
            outline_target: None,
            outline_composite,
            glow_composite,
            glow_composite_single,
            draw_calls: 0,
            pass_count: 0,
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
            // UVs use logical asset dimensions; food may provide a denser raster.
            // Admit only the existing raster or the food body's uniform 4x raster.
            || !([r.image.width, r.image.height] == [a.width, a.height]
                || [r.image.width, r.image.height] == [a.width * 4, a.height * 4])
        {
            return Err("invalid resource".into());
        }
        let mip_level_count = r.image.mip_level_count()?;
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
        let retained_arrays: BTreeMap<_, _> = self
            .textures
            .values()
            .filter(|t| t.asset.handle != a.handle)
            .map(|t| (t.array.key, &t.array.texture))
            .collect();
        let retained = retained_arrays
            .values()
            .map(|t| {
                let (mut width, mut height) = (t.width(), t.height());
                let mut bytes = 0;
                for _ in 0..t.mip_level_count() {
                    bytes +=
                        width as usize * height as usize * 4 * t.depth_or_array_layers() as usize;
                    width = (width / 2).max(1);
                    height = (height / 2).max(1);
                }
                bytes
            })
            .sum::<usize>();
        if retained + r.image.pixels.len() > crate::bank::PIXEL_BUDGET {
            return Err("Resident GPU pixel budget".into());
        }
        let array = self.new_array([r.image.width, r.image.height], 1, mip_level_count)?;
        array.write(&self.queue, 0, r);
        self.arrays.clear();
        // A new revision retires all prior versions of this handle atomically after validation/upload.
        self.textures.retain(|k, _| k.handle != key.handle);
        self.textures.insert(
            key,
            Texture {
                asset: a,
                array,
                layer: 0,
                food_glow: r.food_glow,
                food_outline: r.food_outline,
                black: r
                    .image
                    .pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .all(|p| p[..3] == [0; 3]),
            },
        );
        self.refresh_bindings();
        Ok(())
    }
    fn refresh_bindings(&mut self) {
        self.bindings = self
            .textures
            .values()
            .map(|t| (t.array.key, t.array.clone()))
            .collect();
    }
    fn new_array(&mut self, size: [u32; 2], layers: u32, mips: u32) -> RenderResult<TextureArray> {
        self.next_array = self
            .next_array
            .checked_add(1)
            .ok_or("image array identity exhausted")?;
        Ok(TextureArray::new(
            &self.device,
            &self.layout,
            ResourceKey {
                handle: self.next_array,
                revision: 0,
            },
            size,
            layers,
            mips,
        ))
    }
    pub fn release(&mut self, key: ResourceKey) -> bool {
        self.arrays.clear();
        let removed = self.textures.remove(&key).is_some();
        self.refresh_bindings();
        removed
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
        if (!self.arrays.is_empty() || self.textures.is_empty())
            && wanted.len() == self.textures.len()
            && resources.iter().all(|r| {
                self.textures.get(&r.asset.into()).is_some_and(|t| {
                    t.asset == r.asset
                        && t.food_glow == r.food_glow
                        && t.food_outline == r.food_outline
                })
            })
        {
            return Ok(());
        }
        let mut sizes = BTreeMap::<(u32, u32, u32), BTreeMap<ResourceKey, &Resource>>::new();
        let max_dimension = wgpu::Limits::downlevel_webgl2_defaults().max_texture_dimension_2d;
        for r in resources {
            let a = r.asset;
            if a.status != BodyAssetStatus::Ready
                || a.width == 0
                || a.height == 0
                || a.width > max_dimension
                || a.height > max_dimension
                || !([r.image.width, r.image.height] == [a.width, a.height]
                    || [r.image.width, r.image.height] == [a.width * 4, a.height * 4])
            {
                return Err("invalid resource".into());
            }
            let mips = r.image.mip_level_count()?;
            if self.textures.get(&a.into()).is_some_and(|t| {
                t.asset != a || t.food_glow != r.food_glow || t.food_outline != r.food_outline
            }) {
                return Err("Resident resource metadata changed".into());
            }
            sizes
                .entry((r.image.width, r.image.height, mips))
                .or_default()
                .insert(a.into(), r);
        }
        let max_layers = self.device.limits().max_texture_array_layers as usize;
        let mut arrays = BTreeMap::new();
        let mut textures = BTreeMap::new();
        let mut writes = Vec::new();
        for ((width, height, mips), members) in sizes {
            let members: Vec<_> = members.into_iter().collect();
            for (index, chunk) in members.chunks(max_layers).enumerate() {
                let group_key = (width, height, mips, index);
                let previous = self
                    .arrays
                    .get(&group_key)
                    .filter(|g| g.keys.len() == chunk.len());
                let array = if let Some(g) = previous {
                    g.array.clone()
                } else {
                    self.new_array([width, height], chunk.len() as u32, mips)?
                };
                let previous = self
                    .arrays
                    .get(&group_key)
                    .filter(|g| g.array.key == array.key);
                let mut keys = Vec::with_capacity(chunk.len());
                for (layer, &(key, r)) in chunk.iter().enumerate() {
                    if previous.is_none_or(|g| g.keys[layer] != key) {
                        writes.push((array.clone(), layer as u32, r));
                    }
                    let black = self.textures.get(&key).map_or_else(
                        || {
                            r.image
                                .pixels
                                .as_chunks::<4>()
                                .0
                                .iter()
                                .all(|p| p[..3] == [0; 3])
                        },
                        |t| t.black,
                    );
                    textures.insert(
                        key,
                        Texture {
                            asset: r.asset,
                            array: array.clone(),
                            layer: layer as u32,
                            black,
                            food_glow: r.food_glow,
                            food_outline: r.food_outline,
                        },
                    );
                    keys.push(key);
                }
                arrays.insert(group_key, Group { array, keys });
            }
        }
        // Every new array exists before any layer is staged; nothing is
        // uploaded if an earlier group fails.
        let writes: Vec<_> = writes.iter().map(|(a, l, r)| (a, *l, *r)).collect();
        TextureArray::write_batch(&self.device, &self.queue, &writes);
        self.arrays = arrays;
        self.textures = textures;
        self.refresh_bindings();
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
        if !self.supported_samples.contains(&samples) {
            return Err("Unsupported world sample count".into());
        }
        self.render_inner(frame, target, false, attachment, samples, true)
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
    /// Retained allocation sizes, sampled outside reported preparation/submission.
    #[cfg(feature = "frame-profile")]
    pub fn diagnostic_vertex_capacities(&self) -> Vec<u64> {
        self.vertices
            .iter()
            .filter_map(|chunk| chunk.buffer.as_ref().map(wgpu::Buffer::size))
            .collect()
    }
    /// Select bounded CPU projection participants for explicit offline diagnostics.
    #[cfg(all(feature = "frame-profile", not(target_arch = "wasm32")))]
    pub fn configure_pellet_workers(&mut self, participants: usize, delay: std::time::Duration) {
        if let Some(pellets) = &mut self.pellets {
            pellets.workers.configure(participants, delay);
        }
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
        if start + 1 >= 32 {
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
        self.render_inner(frame, target, true, frame.extent, 1, false)
    }
    fn render_inner(
        &mut self,
        frame: &Frame,
        target: &wgpu::TextureView,
        overlay: bool,
        attachment: [u32; 2],
        samples: u32,
        clear: bool,
    ) -> RenderResult<()> {
        if self.lost.load(Ordering::SeqCst) {
            return Err(RenderError::Host(
                "GPU device lost; no gameplay reset/retry".into(),
            ));
        }
        if frame.food_batch.is_some() {
            frame.check_limits()?;
            if frame.extent.contains(&0) {
                return Ok(());
            }
            if overlay
                || !clear
                || frame.food_batch.as_ref().is_some_and(|b| b.early_at != 0)
                || !matches!(
                    self.format,
                    wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm
                )
            {
                return self.render_inner(
                    &frame.canonical()?,
                    target,
                    overlay,
                    attachment,
                    samples,
                    clear,
                );
            }
            crate::world_resolution::validate_world_target(
                attachment,
                samples,
                self.device.limits().max_texture_dimension_2d,
                self.anti_aliasing,
            )?;
            self.validate_frame(frame)?;
            let mut pellets = self.pellets.take().expect("pellet renderer");
            let result = pellets.render(self, frame, target, attachment, samples);
            self.pellets = Some(pellets);
            return result;
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
        crate::world_resolution::validate_world_target(
            attachment,
            samples,
            if overlay {
                // Preserve the existing physical surface overlay guard. Native
                // world allocation separately checks negotiated texture limits.
                crate::frame::MAX_EXTENT
            } else {
                self.device.limits().max_texture_dimension_2d
            },
            if overlay {
                slither_client::settings::AntiAliasing::Auto
            } else {
                self.anti_aliasing
            },
        )?;
        let chunk_count = frame.draws.len().div_ceil(VERTEX_CHUNK_QUADS);
        self.vertices
            .resize_with(chunk_count.max(self.vertices.len()), VertexChunk::default);
        for chunk in &mut self.vertices {
            chunk.bytes.clear();
            chunk.states.clear();
        }
        let encoded_unorm8 = matches!(
            self.format,
            wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm
        );
        let soft_layers = !overlay && clear && encoded_unorm8;
        let mut glow_span: Option<std::ops::Range<usize>> = None;
        let mut glow_count = 0;
        let mut glow_coverage = [0.; 4];
        let mut outline_coverage = 0.;
        let mut black_prefix = 0;
        let previous =
            if self.layer_history.extent == attachment && self.layer_history.samples == samples {
                self.layer_history
            } else {
                LayerHistory::default()
            };
        let mut outline_span: Option<std::ops::Range<usize>> = None;
        let mut outline_count = 0;
        let mut unsafe_prefix = None;
        let mut quads = quads.peekable();
        let mut uploaded = 0;
        while let Some(quad) = quads.next() {
            let quad = quad?;
            let index = uploaded;
            uploaded += 1;
            let mut second_alpha = 0.;
            let mut paired = false;
            // UNORM8 is the product's encoded blend domain. Other diagnostic
            // formats keep separate additions rather than inheriting this quantizer.
            if encoded_unorm8
                && let Some(Ok(next)) = quads.peek()
                && additive_pair(&quad, next)
            {
                second_alpha = next.vertices[0][4];
                paired = true;
                quads.next();
            }
            let mut scale = 0;
            if soft_layers && self.textures[&quad.key].food_outline {
                outline_span.get_or_insert(index..index).end = index + 1;
                outline_count += 1;
                outline_coverage += covered_area(&quad.vertices);
            } else {
                let covers = quad.vertices[0][0] <= -1.
                    && quad.vertices[0][1] >= 1.
                    && quad.vertices[1][0] >= 1.
                    && quad.vertices[1][1] >= 1.
                    && quad.vertices[2][0] >= 1.
                    && quad.vertices[2][1] <= -1.
                    && quad.vertices[5][0] <= -1.
                    && quad.vertices[5][1] <= -1.;
                if !covers {
                    unsafe_prefix.get_or_insert(index);
                }
            }
            if soft_layers
                && self.textures[&quad.key].food_glow
                && quad.blend == slither_client::presentation::BodyBlend::Lighter
            {
                scale = glow_bin(&quad, second_alpha, attachment);
                if scale > 0 {
                    glow_coverage[scale as usize] += covered_area(&quad.vertices);
                }
                glow_span.get_or_insert(index..index).end = index + 1;
                glow_count += 1;
            }
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
            let texture = &self.textures[&quad.key];
            // Only the opaque black world clear is known here. Black source-over
            // cannot change it (including fs_black's clamped subtraction). A
            // loaded overlay or a later shadow must retain its ordinary draw.
            if soft_layers
                && index == black_prefix
                && texture.black
                && quad.blend == slither_client::presentation::BodyBlend::SourceOver
            {
                black_prefix += 1;
            }
            for vertex in [
                quad.vertices[0],
                quad.vertices[1],
                quad.vertices[2],
                quad.vertices[5],
            ] {
                for value in vertex {
                    chunk.bytes.extend_from_slice(&value.to_le_bytes());
                }
                chunk.bytes.extend_from_slice(&second_alpha.to_le_bytes());
                chunk.bytes.extend_from_slice(&texture.layer.to_le_bytes());
            }
            let mut state = DrawState::new(&quad, self.textures[&quad.key].black);
            state.glow_scale = scale;
            if paired {
                state.noop = zero_pair(quad.vertices[0][4], second_alpha);
                state.pipeline = Pipeline::PairLighter;
            }
            state.key = texture.array.key;
            chunk.states.push(state);
        }
        let glow_bins = select_glows(glow_coverage, samples, previous);
        let glow_span = glow_span.filter(|span| {
            span.end > black_prefix && span.len() == glow_count && glow_bins[1..].contains(&true)
        });
        // An individually unprofitable bin stays on the ordinary path even
        // when another bin can pay for splitting the world pass.
        for chunk in &mut self.vertices[..chunk_count] {
            for state in &mut chunk.states {
                if !glow_bins[state.glow_scale as usize] {
                    state.glow_scale = 0;
                }
            }
        }
        if let Some(span) = &glow_span {
            for (i, chunk) in self.vertices[..chunk_count].iter_mut().enumerate() {
                let base = i * VERTEX_CHUNK_QUADS;
                let start = span.start.saturating_sub(base).min(chunk.states.len());
                let end = span.end.saturating_sub(base).min(chunk.states.len());
                self.glow_sort.clear();
                for j in start..end {
                    self.glow_sort.push((
                        chunk.states[j],
                        chunk.bytes[j * QUAD_VERTEX_BYTES..(j + 1) * QUAD_VERTEX_BYTES]
                            .try_into()
                            .expect("complete quad"),
                    ));
                }
                // Only the contiguous food glow barrier is additive throughout.
                // Nonnegative additive layers commute; bodies and source-over
                // geometry retain their original primitive order.
                self.glow_sort.sort_unstable_by_key(|(state, _)| {
                    (
                        state.glow_scale,
                        state.key,
                        state.pipeline as u8,
                        state.repeat,
                    )
                });
                let mut group_start = 0;
                // Byte-quantized additive pairs can group zero work at the end
                // without fragmenting active runs, then group source layers for
                // texture-cache locality. Non-paired groups retain their order.
                // The authored readback study bounds any GPU rounding drift.
                while group_start < self.glow_sort.len() {
                    let first = self.glow_sort[group_start].0;
                    let mut group_end = group_start + 1;
                    while group_end < self.glow_sort.len() {
                        let next = self.glow_sort[group_end].0;
                        if (first.glow_scale, first.key, first.pipeline, first.repeat)
                            != (next.glow_scale, next.key, next.pipeline, next.repeat)
                        {
                            break;
                        }
                        group_end += 1;
                    }
                    if first.pipeline == Pipeline::PairLighter {
                        self.glow_sort[group_start..group_end].sort_unstable_by_key(
                            |(state, bytes)| {
                                (
                                    state.noop,
                                    u32::from_le_bytes(
                                        bytes[24..28].try_into().expect("layer bytes"),
                                    ),
                                )
                            },
                        );
                    }
                    group_start = group_end;
                }
                for (j, (state, bytes)) in (start..end).zip(self.glow_sort.iter()) {
                    chunk.states[j] = *state;
                    chunk.bytes[j * QUAD_VERTEX_BYTES..(j + 1) * QUAD_VERTEX_BYTES]
                        .copy_from_slice(bytes);
                }
            }
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

        // Keep sharp edges at the world sample count. Only one contiguous,
        // explicitly marked additive glow barrier can use a smaller attachment.
        let outline_span = outline_span.filter(|span| {
            samples > 1
                && span.len() == outline_count
                && unsafe_prefix.is_none_or(|i| i >= span.end)
                && pays(
                    outline_coverage * f64::from(samples - 1),
                    f64::from(samples) + 2.,
                    previous.outline,
                )
        });
        let admitted_outline = outline_span.is_some();
        let outline_span = outline_span.filter(|span| span.end > black_prefix);
        let outline_replacement = outline_span.as_ref().and_then(|_| {
            self.outline_target
                .as_ref()
                .is_none_or(|t| t.extent != attachment)
                .then(|| LayerTarget::new(&self.device, &self.layout, self.format, attachment))
        });
        #[cfg(feature = "frame-profile")]
        let outline_queries = if outline_span.is_some() {
            Some(self.diagnostic_pass("food-outlines")?)
        } else {
            None
        };
        #[cfg(not(feature = "frame-profile"))]
        let outline_queries = Some((0, 1));
        let glow_replacement = glow_span.as_ref().filter(|_| glow_bins[3]).and_then(|_| {
            let extent = attachment.map(|n| n.div_ceil(8));
            self.glow
                .as_ref()
                .is_none_or(|g| g.extent != extent)
                .then(|| LayerTarget::new(&self.device, &self.layout, self.format, extent))
        });
        let fine_replacement = glow_span.as_ref().filter(|_| glow_bins[2]).and_then(|_| {
            let extent = attachment.map(|n| n.div_ceil(4));
            self.fine_glow
                .as_ref()
                .is_none_or(|g| g.extent != extent)
                .then(|| LayerTarget::new(&self.device, &self.layout, self.format, extent))
        });
        let half_replacement = glow_span.as_ref().filter(|_| glow_bins[1]).and_then(|_| {
            let extent = attachment.map(|n| n.div_ceil(2));
            self.half_glow
                .as_ref()
                .is_none_or(|g| g.extent != extent)
                .then(|| LayerTarget::new(&self.device, &self.layout, self.format, extent))
        });
        #[cfg(feature = "frame-profile")]
        let queries = {
            let mut q = vec![self.diagnostic_pass(if overlay { "overlay" } else { "world" })?];
            if glow_span.is_some() {
                if glow_bins[3] {
                    q.push(self.diagnostic_pass("food-glows-coarse")?);
                }
                if glow_bins[2] {
                    q.push(self.diagnostic_pass("food-glows-fine")?);
                }
                if glow_bins[1] {
                    q.push(self.diagnostic_pass("food-glows-half")?);
                }
            }
            q
        };
        #[cfg(not(feature = "frame-profile"))]
        let queries = [(0, 1); 5];
        let msaa_view = (samples > 1).then(|| {
            &replacement
                .as_ref()
                .or(self.multisample.as_ref())
                .expect("MSAA target")
                .view
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        #[cfg(feature = "frame-profile")]
        let aggregate_queries =
            self.diagnostic_timestamps.is_some() && self.diagnostic_passes.is_none();
        #[cfg(not(feature = "frame-profile"))]
        let aggregate_queries = self.diagnostic_timestamps.is_some();
        if aggregate_queries {
            encoder.write_timestamp(self.diagnostic_timestamps.as_ref().expect("queries"), 0);
        }
        let sprite = outline_replacement
            .as_ref()
            .or(self.outline_target.as_ref());
        let mut outline_calls = 0;
        let start = outline_span
            .as_ref()
            .map_or(black_prefix, |span| span.end.max(black_prefix));
        if outline_span.is_some() {
            outline_calls = self.encode_pass(
                &mut encoder,
                PaintPass {
                    target: &sprite.expect("sprite target").view,
                    resolve: None,
                    samples: 1,
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    before: None,
                    range: black_prefix..start,
                    queries: if aggregate_queries {
                        None
                    } else {
                        outline_queries
                    },
                    coarse: None,
                    fine: [None; 2],
                    small_glows: None,
                    filter: None,
                    base: None,
                },
            );
        }
        let base = outline_span
            .as_ref()
            .map(|_| &sprite.expect("sprite target").bind);
        let world = msaa_view.unwrap_or(target);
        let has_glows = glow_span.is_some();
        let pass_count = 1
            + usize::from(outline_span.is_some())
            + if has_glows {
                glow_bins[1..].iter().filter(|&&active| active).count()
            } else {
                0
            };
        let query = |i| (!aggregate_queries).then_some(queries[i]);
        let draw_calls = if let Some(span) = glow_span {
            // Reduced targets read source rasters only. Paint them first, then
            // keep all world geometry/composites in the original order within
            // one MSAA pass, avoiding the intermediate world store/load.
            let mut soft = 0;
            let mut i = 1;
            let coarse = glow_bins[3].then(|| {
                glow_replacement
                    .as_ref()
                    .or(self.glow.as_ref())
                    .expect("coarse glow target")
            });
            if let Some(glow) = coarse {
                soft += self.encode_pass(
                    &mut encoder,
                    PaintPass {
                        target: &glow.view,
                        resolve: None,
                        samples: 1,
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        before: None,
                        range: span.clone(),
                        queries: query(i),
                        coarse: None,
                        fine: [None; 2],
                        small_glows: None,
                        filter: Some(3),
                        base: None,
                    },
                );
                i += 1;
            }
            let fine = glow_bins[2].then(|| {
                fine_replacement
                    .as_ref()
                    .or(self.fine_glow.as_ref())
                    .expect("fine glow target")
            });
            if let Some(glow) = fine {
                soft += self.encode_pass(
                    &mut encoder,
                    PaintPass {
                        target: &glow.view,
                        resolve: None,
                        samples: 1,
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        before: None,
                        range: span.clone(),
                        queries: query(i),
                        coarse: None,
                        fine: [None; 2],
                        small_glows: None,
                        filter: Some(2),
                        base: None,
                    },
                );
                i += 1;
            }
            let half = glow_bins[1].then(|| {
                half_replacement
                    .as_ref()
                    .or(self.half_glow.as_ref())
                    .expect("half glow target")
            });
            if let Some(glow) = half {
                soft += self.encode_pass(
                    &mut encoder,
                    PaintPass {
                        target: &glow.view,
                        resolve: None,
                        samples: 1,
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        before: None,
                        range: span.clone(),
                        queries: query(i),
                        coarse: None,
                        fine: [None; 2],
                        small_glows: None,
                        filter: Some(1),
                        base: None,
                    },
                );
            }
            let after = self.encode_pass(
                &mut encoder,
                PaintPass {
                    target: world,
                    resolve: msaa_view.map(|_| target),
                    samples,
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    before: Some(start..span.start),
                    range: span.end..frame.draws.len(),
                    queries: query(0),
                    coarse: coarse.map(|g| &g.bind),
                    fine: [half.map(|g| &g.bind), fine.map(|g| &g.bind)],
                    small_glows: Some(span.clone()),
                    filter: None,
                    base,
                },
            );
            soft + after
        } else {
            self.encode_pass(
                &mut encoder,
                PaintPass {
                    target: world,
                    resolve: msaa_view.map(|_| target),
                    samples,
                    load: if overlay || !clear {
                        wgpu::LoadOp::Load
                    } else {
                        wgpu::LoadOp::Clear(wgpu::Color::BLACK)
                    },
                    before: None,
                    range: start..frame.draws.len(),
                    queries: query(0),
                    coarse: None,
                    fine: [None; 2],
                    small_glows: None,
                    filter: None,
                    base,
                },
            )
        };
        if aggregate_queries {
            encoder.write_timestamp(self.diagnostic_timestamps.as_ref().expect("queries"), 1);
        }
        let submission = crate::latency::submit_begin();
        self.queue.submit([encoder.finish()]);
        crate::latency::submit_end(submission);
        self.draw_calls = draw_calls + outline_calls;
        self.pass_count = pass_count;
        if !overlay {
            self.layer_history = LayerHistory {
                extent: attachment,
                samples,
                outline: admitted_outline,
                glow: has_glows,
                glow_bins: if has_glows { glow_bins } else { [false; 4] },
            };
        }
        if let Some(sprite) = outline_replacement {
            self.outline_target = Some(sprite);
        }
        if !overlay && outline_span.is_none() {
            self.outline_target = None;
        }
        if let Some(fine) = fine_replacement {
            self.fine_glow = Some(fine);
        }
        if let Some(half) = half_replacement {
            self.half_glow = Some(half);
        }
        if let Some(glow) = glow_replacement {
            self.glow = Some(glow);
        }
        if !overlay && !has_glows {
            self.glow = None;
            self.fine_glow = None;
            self.half_glow = None;
        }
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
    fn encode_draws<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        range: std::ops::Range<usize>,
        samples: u32,
        filter: Option<u8>,
    ) -> usize {
        let mut draw_calls = 0;
        let mut previous: Option<DrawState> = None;
        pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint16);
        for (i, chunk) in self.vertices.iter().enumerate() {
            let base = i * VERTEX_CHUNK_QUADS;
            let start = range.start.saturating_sub(base).min(chunk.states.len());
            let end = range.end.saturating_sub(base).min(chunk.states.len());
            if start >= end {
                continue;
            }
            pass.set_vertex_buffer(0, chunk.buffer.as_ref().expect("vertex buffer").slice(..));
            for (state, vertices) in draw_runs(chunk.states[start..end].iter().copied()) {
                if state.noop {
                    continue;
                }
                if filter.is_some_and(|coarse| coarse != state.glow_scale) {
                    continue;
                }
                if previous.is_none_or(|p| p.pipeline != state.pipeline) {
                    pass.set_pipeline(match state.pipeline {
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
                    let texture = &self.bindings[&state.key];
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
                pass.draw_indexed(
                    vertices.start + start as u32 * 6..vertices.end + start as u32 * 6,
                    0,
                    0..1,
                );
                draw_calls += 1;
                previous = Some(state);
            }
        }
        draw_calls
    }
    fn encode_pass(&self, encoder: &mut wgpu::CommandEncoder, plan: PaintPass<'_>) -> usize {
        let PaintPass {
            before,
            target,
            resolve,
            samples,
            load,
            range,
            queries,
            coarse,
            fine,
            small_glows,
            filter,
            base,
        } = plan;
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ordered world layer"),
            timestamp_writes: self.diagnostic_timestamps.as_ref().zip(queries).map(
                |(query_set, queries)| wgpu::RenderPassTimestampWrites {
                    query_set,
                    beginning_of_pass_write_index: Some(queries.0),
                    end_of_pass_write_index: Some(queries.1),
                },
            ),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: resolve,
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        if let Some(bind) = base {
            pass.set_pipeline(&self.outline_composite[&samples]);
            pass.set_bind_group(0, bind, &[]);
            pass.draw(0..3, 0..1);
        }
        let before_draws = before.map_or(0, |range| {
            self.encode_draws(&mut pass, range, samples, filter)
        });
        let mut small_draws = 0;
        for bind in fine.into_iter().flatten() {
            pass.set_pipeline(if samples == 1 {
                &self.fine_composite_single
            } else {
                &self.outline_composite[&samples]
            });
            pass.set_bind_group(0, bind, &[]);
            pass.draw(0..3, 0..1);
        }
        if let Some(bind) = coarse {
            pass.set_pipeline(if samples == 1 {
                &self.glow_composite_single
            } else {
                &self.glow_composite[&samples]
            });
            pass.set_bind_group(0, bind, &[]);
            pass.draw(0..3, 0..1);
        }
        if let Some(range) = small_glows {
            small_draws = self.encode_draws(&mut pass, range, samples, Some(0));
        }
        self.encode_draws(&mut pass, range, samples, filter)
            + small_draws
            + before_draws
            + usize::from(coarse.is_some())
            + fine.iter().flatten().count()
            + usize::from(base.is_some())
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
        top: Option<&Frame>,
    ) -> RenderResult<bool> {
        if top.is_some_and(|f| f.extent != self.extent) {
            return Err("menu layer/surface extent mismatch".into());
        }
        self.present_inner_with(frame, true, overlay, requested, |renderer, view| {
            menus.paint(renderer, view);
            // The studio preview stays above the egui panel it sits in.
            top.map_or(Ok(()), |top| renderer.render_overlay(top, view))
        })
    }
    fn present_inner(
        &mut self,
        frame: &Frame,
        scale: bool,
        overlay: Option<&Frame>,
        requested: slither_client::settings::WorldResolution,
    ) -> RenderResult<bool> {
        self.present_inner_with(frame, scale, overlay, requested, |_, _| Ok(()))
    }
    fn present_inner_with(
        &mut self,
        frame: &Frame,
        scale: bool,
        overlay: Option<&Frame>,
        requested: slither_client::settings::WorldResolution,
        paint: impl FnOnce(&mut Renderer, &wgpu::TextureView) -> RenderResult<()>,
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
        self.renderer
            .world_plan(requested, frame.extent, self.extent)?;
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
        paint(&mut self.renderer, &view)?;
        crate::latency::picture("present_begin");
        self.renderer.queue.present(texture);
        crate::latency::picture("present_end");
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_arch = "wasm32"))]
    fn read_layer(r: &Renderer, texture: &wgpu::Texture, layer: u32, level: u32) -> Vec<u8> {
        let (width, height) = (
            (texture.width() >> level).max(1),
            (texture.height() >> level).max(1),
        );
        let pitch = (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let read = r.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: u64::from(pitch * height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = r.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: level,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: layer,
                },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &read,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(pitch),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        r.queue.submit([encoder.finish()]);
        read.slice(..)
            .map_async(wgpu::MapMode::Read, |r| r.unwrap());
        r.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .unwrap();
        let bytes = read.slice(..).get_mapped_range().unwrap();
        bytes
            .chunks(pitch as usize)
            .flat_map(|row| &row[..width as usize * 4])
            .copied()
            .collect()
    }

    // Authored random texels; no game art. Odd, aligned and one-pixel widths
    // cover row padding, whole-level copies and every mip of each chain.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    #[ignore = "explicit finite offscreen GPU check"]
    fn batched_layer_uploads_match_per_call_writes() {
        let adapter = pollster::block_on(instance().request_adapter(&Default::default())).unwrap();
        let mut r = pollster::block_on(Renderer::from_adapter_with_max_samples(
            adapter,
            wgpu::TextureFormat::Rgba8Unorm,
            1,
        ))
        .unwrap();
        let scope = r.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let mut seed = 0x9e37_79b9_u32;
        for (width, height, full_chain) in [
            (1, 1, true),
            (37, 11, true),
            (64, 64, true),
            (130, 3, false),
            (5, 96, true),
        ] {
            let mut bytes = 0;
            let (mut w, mut h) = (width, height);
            let levels = if full_chain {
                u32::max(width, height).ilog2() + 1
            } else {
                2
            };
            for _ in 0..levels {
                bytes += w as usize * h as usize * 4;
                (w, h) = ((w / 2).max(1), (h / 2).max(1));
            }
            let resources: Vec<Resource> = (0..3)
                .map(|i| {
                    let pixels = (0..bytes)
                        .map(|_| {
                            seed ^= seed << 13;
                            seed ^= seed >> 17;
                            seed ^= seed << 5;
                            seed as u8
                        })
                        .collect();
                    Resource {
                        food_glow: false,
                        food_outline: false,
                        asset: BodyAsset {
                            handle: 1 + i,
                            revision: 1,
                            width,
                            height,
                            status: BodyAssetStatus::Ready,
                        },
                        image: crate::materials::RgbaImage {
                            width,
                            height,
                            pixels,
                        }
                        .into(),
                    }
                })
                .collect();
            let mips = resources[0].image.mip_level_count().unwrap();
            assert_eq!(mips, levels);
            let batched = r.new_array([width, height], 3, mips).unwrap();
            let serial = r.new_array([width, height], 3, mips).unwrap();
            let writes: Vec<_> = resources
                .iter()
                .enumerate()
                .map(|(i, x)| (&batched, i as u32, x))
                .collect();
            TextureArray::write_batch(&r.device, &r.queue, &writes);
            for (i, x) in resources.iter().enumerate() {
                serial.write(&r.queue, i as u32, x);
            }
            for layer in 0..3 {
                let mut offset = 0;
                for level in 0..mips {
                    let expected = read_layer(&r, &serial.texture, layer, level);
                    assert_eq!(
                        expected,
                        resources[layer as usize].image.pixels[offset..offset + expected.len()]
                    );
                    assert_eq!(read_layer(&r, &batched.texture, layer, level), expected);
                    offset += expected.len();
                }
            }
        }
        assert!(pollster::block_on(scope.pop()).is_none());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_renderer_retains_send_ownership() {
        fn send<T: Send>() {}
        send::<Renderer>();
    }

    #[test]
    fn chunk_and_record_storage_fit_webgl2_device_limits() {
        let limits = wgpu::Limits::downlevel_webgl2_defaults();
        assert_eq!(std::mem::size_of::<crate::frame::Draw>(), 256);
        assert_eq!(std::mem::size_of::<DrawState>(), 24);
        assert_eq!(crate::frame::MAX_DRAWS, 262_144);
        assert_eq!(VERTEX_CHUNK_QUADS * QUAD_VERTEX_BYTES, 917_504);
        assert!((VERTEX_CHUNK_QUADS * QUAD_VERTEX_BYTES) as u64 <= limits.max_buffer_size);
        assert!(28 <= limits.max_vertex_buffer_array_stride);
        assert!(3 <= limits.max_vertex_attributes);
        assert!(1 <= limits.max_vertex_buffers);
        // Retained pellets use only instanced vertex inputs and uniforms, within
        // the same downlevel contract (no storage buffers or optional features).
        assert!(7 <= limits.max_vertex_attributes);
        assert!(2 <= limits.max_vertex_buffers);
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
                food_glow: false,
                food_outline: false,
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
            food_batch: None,
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
