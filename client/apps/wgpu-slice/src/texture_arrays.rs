//! Equal-size images share a binding; each layer retains its own clamp/repeat edges.
use crate::{frame::ResourceKey, scene::Resource};
use std::sync::Arc;

pub(super) struct TextureArray {
    pub key: ResourceKey,
    pub texture: wgpu::Texture,
    pub bind: wgpu::BindGroup,
    pub repeat_bind: wgpu::BindGroup,
}
pub(super) struct Group {
    pub array: Arc<TextureArray>,
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
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
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
