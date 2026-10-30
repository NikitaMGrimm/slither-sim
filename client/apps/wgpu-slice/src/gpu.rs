//! Shared WebGPU/DX12/Vulkan texture-quad renderer. No sorting, game clock or Canvas fallback.
use crate::{
    Result,
    frame::{Frame, ResourceKey},
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
use wgpu::util::DeviceExt;

#[path = "draw_runs.rs"]
mod draw_runs;
use draw_runs::{DrawState, Pipeline, draw_runs};

struct Texture {
    asset: BodyAsset,
    _texture: wgpu::Texture,
    bind: wgpu::BindGroup,
    repeat_bind: wgpu::BindGroup,
    black: bool,
}
pub struct Renderer {
    pub sample_count: u32,
    format: wgpu::TextureFormat,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter_info: wgpu::AdapterInfo,
    layout: wgpu::BindGroupLayout,
    over: wgpu::RenderPipeline,
    lighter: wgpu::RenderPipeline,
    black_over: wgpu::RenderPipeline,
    textures: BTreeMap<ResourceKey, Texture>,
    lost: Arc<AtomicBool>,
}
impl Renderer {
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
    async fn from_adapter(adapter: wgpu::Adapter, format: wgpu::TextureFormat) -> Result<Self> {
        let info = adapter.get_info();
        let supported = adapter.get_texture_format_features(format).flags;
        let sample_count = if supported.contains(
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
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
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
            |blend, fragment| {
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
                multisample: wgpu::MultisampleState{count:sample_count,..Default::default()},
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
        let over = pipeline(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING, "fs");
        let black_over = pipeline(
            wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::ReverseSubtract,
                },
                alpha: wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING.alpha,
            },
            "fs_black",
        );
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
        );
        Ok(Self {
            sample_count,
            format,
            device,
            queue,
            adapter_info: info,
            layout,
            over,
            lighter,
            black_over,
            textures: BTreeMap::new(),
            lost,
        })
    }
    pub fn upload(&mut self, r: &Resource) -> Result<()> {
        let a = r.asset;
        let key = ResourceKey::from(a);
        if self.lost.load(Ordering::SeqCst) {
            return Err("GPU device lost; host stopped, scene retained".into());
        }
        if a.status != BodyAssetStatus::Ready
            || a.width == 0
            || a.height == 0
            || a.width > 512
            || a.height > 512
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
    pub fn sync_resources(&mut self, resources: &[Resource]) -> Result<()> {
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
    pub fn render(&self, frame: &Frame, target: &wgpu::TextureView) -> Result<()> {
        if self.lost.load(Ordering::SeqCst) {
            return Err("GPU device lost; no gameplay reset/retry".into());
        }
        let resident = self
            .textures
            .iter()
            .map(|(key, t)| (*key, t.asset))
            .collect();
        let quads = frame.quads(&resident)?;
        if frame.extent.contains(&0) {
            return Ok(());
        }
        if u64::from(frame.extent[0])
            * u64::from(frame.extent[1])
            * 4
            * u64::from(self.sample_count)
            > 128 * 1024 * 1024
        {
            return Err("Render attachment budget".into());
        }
        let bytes: Vec<u8> = quads
            .iter()
            .flat_map(|q| {
                q.vertices
                    .iter()
                    .flat_map(|v| v.iter().flat_map(|f| f.to_le_bytes()))
            })
            .collect();
        let buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("ordered vertices"),
                contents: if bytes.is_empty() { &[0; 4] } else { &bytes },
                usage: wgpu::BufferUsages::VERTEX,
            });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let multisample = if self.sample_count > 1 {
            Some(self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("quad edge coverage"),
                size: wgpu::Extent3d {
                    width: frame.extent[0],
                    height: frame.extent[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: self.sample_count,
                dimension: wgpu::TextureDimension::D2,
                format: self.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            }))
        } else {
            None
        };
        let msaa_view = multisample
            .as_ref()
            .map(|t| t.create_view(&Default::default()));
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("offline gameplay"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: msaa_view.as_ref().unwrap_or(target),
                    depth_slice: None,
                    resolve_target: msaa_view.as_ref().map(|_| target),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.035,
                            g: 0.045,
                            b: 0.06,
                            a: 1.,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_vertex_buffer(0, buffer.slice(..));
            let states = quads
                .iter()
                .map(|q| DrawState::new(q, self.textures[&q.key].black));
            let mut previous: Option<DrawState> = None;
            for (state, vertices) in draw_runs(states) {
                if previous.is_none_or(|p| p.pipeline != state.pipeline) {
                    pass.set_pipeline(match state.pipeline {
                        Pipeline::BlackOver => &self.black_over,
                        Pipeline::Lighter => &self.lighter,
                        Pipeline::Over => &self.over,
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
                previous = Some(state);
            }
        }
        self.queue.submit([encoder.finish()]);
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
    backing: Option<wgpu::Texture>,
    blitter: wgpu::util::TextureBlitter,
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
        let blitter = wgpu::util::TextureBlitterBuilder::new(&renderer.device, format)
            .sample_type(wgpu::FilterMode::Linear)
            .build();
        let mut host = Self {
            renderer,
            surface,
            config,
            extent: [0, 0],
            backing: None,
            blitter,
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
        self.present_inner(frame, false)
    }
    /// Render at the desktop policy size, then scale the complete image like a CSS canvas.
    pub fn present_scaled(&mut self, frame: &Frame) -> Result<bool> {
        self.present_inner(frame, true)
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
        self.acquired = Some(match self.surface.get_current_texture() {
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
    fn present_inner(&mut self, frame: &Frame, scale: bool) -> Result<bool> {
        let scaled = frame.extent != self.extent;
        if scaled && !scale {
            return Err("frame/surface extent mismatch".into());
        }
        if self.extent.contains(&0) || frame.extent.contains(&0) {
            return Ok(false);
        }
        if scaled {
            if frame.extent.iter().any(|n| *n > 1500) {
                return Err("Desktop backing limit".into());
            }
            if self
                .backing
                .as_ref()
                .is_none_or(|t| [t.width(), t.height()] != frame.extent)
            {
                self.backing = Some(self.renderer.device.create_texture(
                    &wgpu::TextureDescriptor {
                        label: Some("desktop policy backing"),
                        size: wgpu::Extent3d {
                            width: frame.extent[0],
                            height: frame.extent[1],
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: self.config.format,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                            | wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    },
                ));
            }
        }
        if !self.acquire()? {
            return Ok(false);
        }
        let texture = self.acquired.take().expect("acquired surface");
        let view = texture.texture.create_view(&Default::default());
        if scaled {
            let backing = self
                .backing
                .as_ref()
                .expect("backing")
                .create_view(&Default::default());
            self.renderer.render(frame, &backing)?;
            let mut encoder = self
                .renderer
                .device
                .create_command_encoder(&Default::default());
            self.blitter
                .copy(&self.renderer.device, &mut encoder, &backing, &view);
            self.renderer.queue.submit([encoder.finish()]);
        } else {
            self.renderer.render(frame, &view)?;
        }
        self.renderer.queue.present(texture);
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
