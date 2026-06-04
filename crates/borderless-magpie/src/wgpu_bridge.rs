// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: Copyright (c) Xu and Magpie contributors
// SPDX-FileCopyrightText: Copyright (c) Borderless Oxide contributors
// Magpie-compatible wgpu descriptor bridge for the GPL comparison path:
// https://github.com/Blinue/Magpie

use crate::backend::{
    MagpieBackendBufferDescriptor, MagpieBackendDescriptorPlan, MagpieBackendSamplerDescriptor,
    MagpieBackendTextureBind, MagpieBackendTextureDescriptor,
};
use crate::magpiefx::{MagpieFxSamplerAddress, MagpieFxSamplerFilter};
use crate::plan::MagpieTextureFormat;
use borderless_upscale_core::{UpscaleError, UpscaleResult};

pub struct MagpieWgpuDescriptorPlan<'a> {
    pub textures: Vec<wgpu::TextureDescriptor<'a>>,
    pub buffers: Vec<wgpu::BufferDescriptor<'a>>,
    pub samplers: Vec<wgpu::SamplerDescriptor<'a>>,
}

impl<'a> MagpieWgpuDescriptorPlan<'a> {
    pub fn from_backend_descriptors(
        descriptors: &'a MagpieBackendDescriptorPlan,
    ) -> UpscaleResult<Self> {
        let textures = descriptors
            .textures
            .iter()
            .map(wgpu_texture_descriptor)
            .collect::<UpscaleResult<Vec<_>>>()?;
        let buffers = descriptors
            .buffers
            .iter()
            .map(wgpu_buffer_descriptor)
            .collect();
        let samplers = descriptors
            .samplers
            .iter()
            .map(wgpu_sampler_descriptor)
            .collect();

        Ok(Self {
            textures,
            buffers,
            samplers,
        })
    }
}

pub fn wgpu_texture_descriptor(
    texture: &MagpieBackendTextureDescriptor,
) -> UpscaleResult<wgpu::TextureDescriptor<'_>> {
    Ok(wgpu::TextureDescriptor {
        label: Some(texture.name.as_str()),
        size: wgpu::Extent3d {
            width: texture.size.width,
            height: texture.size.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu_texture_format(&texture.format.format)?,
        usage: wgpu_texture_usages(&texture.bind_flags),
        view_formats: &[],
    })
}

#[must_use]
pub fn wgpu_buffer_descriptor(
    buffer: &MagpieBackendBufferDescriptor,
) -> wgpu::BufferDescriptor<'_> {
    wgpu::BufferDescriptor {
        label: Some(buffer.name.as_str()),
        size: buffer.byte_len as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    }
}

#[must_use]
pub fn wgpu_sampler_descriptor(
    sampler: &MagpieBackendSamplerDescriptor,
) -> wgpu::SamplerDescriptor<'_> {
    let address = wgpu_address_mode(sampler.address);
    let filter = wgpu_filter_mode(sampler.filter);
    wgpu::SamplerDescriptor {
        label: Some(sampler.name.as_str()),
        address_mode_u: address,
        address_mode_v: address,
        address_mode_w: address,
        mag_filter: filter,
        min_filter: filter,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        lod_min_clamp: 0.0,
        lod_max_clamp: 32.0,
        compare: None,
        anisotropy_clamp: 1,
        border_color: None,
    }
}

fn wgpu_texture_format(format: &MagpieTextureFormat) -> UpscaleResult<wgpu::TextureFormat> {
    match format {
        MagpieTextureFormat::R8Unorm => Ok(wgpu::TextureFormat::R8Unorm),
        MagpieTextureFormat::R8g8Unorm => Ok(wgpu::TextureFormat::Rg8Unorm),
        MagpieTextureFormat::R8g8b8a8Unorm => Ok(wgpu::TextureFormat::Rgba8Unorm),
        MagpieTextureFormat::R8g8b8a8UnormSrgb => Ok(wgpu::TextureFormat::Rgba8UnormSrgb),
        MagpieTextureFormat::R8g8b8a8Snorm => Ok(wgpu::TextureFormat::Rgba8Snorm),
        MagpieTextureFormat::R16Float => Ok(wgpu::TextureFormat::R16Float),
        MagpieTextureFormat::R16g16Float => Ok(wgpu::TextureFormat::Rg16Float),
        MagpieTextureFormat::R16g16b16a16Float => Ok(wgpu::TextureFormat::Rgba16Float),
        MagpieTextureFormat::R32Float => Ok(wgpu::TextureFormat::R32Float),
        MagpieTextureFormat::R32g32b32a32Float => Ok(wgpu::TextureFormat::Rgba32Float),
        MagpieTextureFormat::Dxgi(value) => Err(invalid_pipeline(format!(
            "DXGI format {value} has no wgpu mapping yet"
        ))),
        MagpieTextureFormat::Unknown(value) => Err(invalid_pipeline(format!(
            "unknown Magpie texture format {value} has no wgpu mapping"
        ))),
    }
}

fn wgpu_texture_usages(bind_flags: &[MagpieBackendTextureBind]) -> wgpu::TextureUsages {
    bind_flags
        .iter()
        .fold(wgpu::TextureUsages::empty(), |usage, bind| match bind {
            MagpieBackendTextureBind::ExternalInput | MagpieBackendTextureBind::SourceUpload => {
                usage | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING
            }
            MagpieBackendTextureBind::ExternalOutput => {
                usage | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::STORAGE_BINDING
            }
            MagpieBackendTextureBind::ShaderResource => {
                usage | wgpu::TextureUsages::TEXTURE_BINDING
            }
            MagpieBackendTextureBind::UnorderedAccess => {
                usage | wgpu::TextureUsages::STORAGE_BINDING
            }
        })
}

fn wgpu_address_mode(address: MagpieFxSamplerAddress) -> wgpu::AddressMode {
    match address {
        MagpieFxSamplerAddress::Clamp => wgpu::AddressMode::ClampToEdge,
        MagpieFxSamplerAddress::Wrap => wgpu::AddressMode::Repeat,
    }
}

fn wgpu_filter_mode(filter: MagpieFxSamplerFilter) -> wgpu::FilterMode {
    match filter {
        MagpieFxSamplerFilter::Point => wgpu::FilterMode::Nearest,
        MagpieFxSamplerFilter::Linear => wgpu::FilterMode::Linear,
    }
}

fn invalid_pipeline(message: impl Into<String>) -> UpscaleError {
    UpscaleError::InvalidPipeline(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{
        MagpieBackendBufferDescriptor, MagpieBackendSamplerDescriptor, MagpieBackendTextureBind,
        MagpieBackendTextureDescriptor,
    };
    use crate::formats::MagpieTextureComponent;
    use crate::formats::MagpieTextureFormatDescriptor;
    use crate::magpiefx::{MagpieFxSamplerAddress, MagpieFxSamplerFilter};
    use crate::plan::MagpieTextureFormat;
    use borderless_upscale_core::FrameSize;

    #[test]
    fn maps_texture_buffer_and_sampler_descriptors_to_wgpu() {
        let texture_binding = texture_descriptor();
        let texture = wgpu_texture_descriptor(&texture_binding).unwrap();
        assert_eq!(texture.format, wgpu::TextureFormat::Rgba8Unorm);
        assert_eq!(texture.size.width, 640);
        assert!(texture.usage.contains(wgpu::TextureUsages::TEXTURE_BINDING));
        assert!(texture.usage.contains(wgpu::TextureUsages::STORAGE_BINDING));

        let buffer_binding = MagpieBackendBufferDescriptor {
            name: "__CB1".to_owned(),
            register: 0,
            byte_len: 64,
            initial_dwords: vec![0; 16],
        };
        let buffer = wgpu_buffer_descriptor(&buffer_binding);
        assert_eq!(buffer.size, 64);
        assert!(buffer.usage.contains(wgpu::BufferUsages::UNIFORM));

        let sampler_binding = MagpieBackendSamplerDescriptor {
            name: "POINT".to_owned(),
            register: 0,
            filter: MagpieFxSamplerFilter::Point,
            address: MagpieFxSamplerAddress::Wrap,
        };
        let sampler = wgpu_sampler_descriptor(&sampler_binding);
        assert_eq!(sampler.mag_filter, wgpu::FilterMode::Nearest);
        assert_eq!(sampler.address_mode_u, wgpu::AddressMode::Repeat);
    }

    #[test]
    fn rejects_unmapped_dxgi_format() {
        let mut texture = texture_descriptor();
        texture.format.format = MagpieTextureFormat::Dxgi(98);

        assert!(matches!(
            wgpu_texture_descriptor(&texture),
            Err(UpscaleError::InvalidPipeline(_))
        ));
    }

    fn texture_descriptor() -> MagpieBackendTextureDescriptor {
        MagpieBackendTextureDescriptor {
            name: "OUTPUT".to_owned(),
            size: FrameSize::new(640, 480).unwrap(),
            format: MagpieTextureFormatDescriptor {
                format: MagpieTextureFormat::R8g8b8a8Unorm,
                dxgi_format: Some(28),
                bytes_per_pixel: Some(4),
                channels: Some(4),
                component: MagpieTextureComponent::Unorm,
                normalized: true,
                signed: false,
                srgb: false,
            },
            dxgi_format: 28,
            bind_flags: vec![
                MagpieBackendTextureBind::ShaderResource,
                MagpieBackendTextureBind::UnorderedAccess,
            ],
        }
    }
}
