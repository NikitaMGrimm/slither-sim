//! Equal-size images share a binding; each layer retains its own clamp/repeat edges.
use crate::{frame::ResourceKey, scene::Resource};

#[derive(Clone)]
pub(super) struct TextureArray {
    pub key: ResourceKey,
    pub texture: wgpu::Texture,
    pub bind: wgpu::BindGroup,
    pub repeat_bind: wgpu::BindGroup,
}
pub(super) struct Group {
    pub array: TextureArray,
    pub keys: Vec<ResourceKey>,
}
impl TextureArray {
    pub fn new(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        key: ResourceKey,
        [width, height]: [u32; 2],
        layers: u32,
        mip_level_count: u32,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("equal-size image array"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: layers,
            },
            mip_level_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            // Unit tests read layers back; production descriptors are unchanged.
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | if cfg!(test) {
                    wgpu::TextureUsages::COPY_SRC
                } else {
                    wgpu::TextureUsages::empty()
                },
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let binding = |address_mode| {
            let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("Canvas-like linear clamp"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                address_mode_u: address_mode,
                address_mode_v: address_mode,
                ..Default::default()
            });
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("image array"),
                layout,
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
        Self {
            key,
            texture,
            bind,
            repeat_bind,
        }
    }

    /// Upload several layers through one mapped staging buffer and one copy
    /// submission. Native `Queue::write_texture` stages each (layer, mip) call
    /// in its own buffer, which made a join's first few hundred sprites cost
    /// tens of milliseconds on an integrated DX12 adapter. Texel bytes, layers
    /// and mip levels are unchanged; only rows are padded inside the staging copy.
    /// See https://github.com/NikitaMGrimm/slither-sim/blob/main/research/client-performance/2026-11-05-batched-uploads/REPORT.md.
    pub fn write_batch(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        writes: &[(&Self, u32, &Resource)],
    ) {
        if cfg!(target_arch = "wasm32") || writes.len() < 2 {
            for (array, layer, resource) in writes {
                array.write(queue, *layer, resource);
            }
            return;
        }
        // Region offsets satisfy the strictest backend placement rule (DX12: 512).
        const PLACEMENT: u64 = 512;
        let row_pitch =
            |width: u32| (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let mut size = 0u64;
        for (array, _, resource) in writes {
            let (mut width, mut height) = (resource.image.width, resource.image.height);
            for _ in 0..array.texture.mip_level_count() {
                size = size.next_multiple_of(PLACEMENT)
                    + u64::from(row_pitch(width)) * u64::from(height);
                width = (width / 2).max(1);
                height = (height / 2).max(1);
            }
        }
        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("batched image upload"),
            size: size.next_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT),
            usage: wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: true,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("batched image upload"),
        });
        {
            let mut mapped = staging
                .slice(..)
                .get_mapped_range_mut()
                .expect("buffer created mapped");
            let mut offset = 0u64;
            for (array, layer, resource) in writes {
                let (mut width, mut height) = (resource.image.width, resource.image.height);
                let mut source = 0;
                for level in 0..array.texture.mip_level_count() {
                    offset = offset.next_multiple_of(PLACEMENT);
                    let (row, pitch) = (width as usize * 4, row_pitch(width));
                    if row == pitch as usize {
                        let bytes = row * height as usize;
                        let at = offset as usize;
                        mapped
                            .slice(at..at + bytes)
                            .copy_from_slice(&resource.image.pixels[source..source + bytes]);
                        source += bytes;
                    } else {
                        for y in 0..height as usize {
                            let at = offset as usize + y * pitch as usize;
                            mapped
                                .slice(at..at + row)
                                .copy_from_slice(&resource.image.pixels[source..source + row]);
                            source += row;
                        }
                    }
                    encoder.copy_buffer_to_texture(
                        wgpu::TexelCopyBufferInfo {
                            buffer: &staging,
                            layout: wgpu::TexelCopyBufferLayout {
                                offset,
                                bytes_per_row: Some(pitch),
                                rows_per_image: Some(height),
                            },
                        },
                        wgpu::TexelCopyTextureInfo {
                            texture: &array.texture,
                            mip_level: level,
                            origin: wgpu::Origin3d {
                                x: 0,
                                y: 0,
                                z: *layer,
                            },
                            aspect: wgpu::TextureAspect::All,
                        },
                        wgpu::Extent3d {
                            width,
                            height,
                            depth_or_array_layers: 1,
                        },
                    );
                    offset += u64::from(pitch) * u64::from(height);
                    width = (width / 2).max(1);
                    height = (height / 2).max(1);
                }
            }
        }
        staging.unmap();
        // Submitted now, ahead of the picture that samples these layers.
        queue.submit([encoder.finish()]);
    }

    pub fn write(&self, queue: &wgpu::Queue, layer: u32, resource: &Resource) {
        let (mut width, mut height) = (resource.image.width, resource.image.height);
        let mut offset = 0;
        for level in 0..self.texture.mip_level_count() {
            let bytes = width as usize * height as usize * 4;
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.texture,
                    mip_level: level,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: layer,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &resource.image.pixels[offset..offset + bytes],
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width * 4),
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
            offset += bytes;
            width = (width / 2).max(1);
            height = (height / 2).max(1);
        }
    }
}
