// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: Copyright (c) Xu and Magpie contributors
// SPDX-FileCopyrightText: Copyright (c) Borderless Oxide contributors
// Magpie-compatible wgpu descriptor bridge for the GPL comparison path:
// https://github.com/Blinue/Magpie

use crate::backend::{
    MagpieBackendBufferDescriptor, MagpieBackendDescriptorPlan, MagpieBackendSamplerDescriptor,
    MagpieBackendTextureBind, MagpieBackendTextureDescriptor,
};
use crate::formats::MagpieTextureComponent;
use crate::magpiefx::{MagpieFxSamplerAddress, MagpieFxSamplerFilter};
use crate::plan::MagpieTextureFormat;
use crate::resources::{
    MagpieConstantBufferBinding, MagpieResourcePass, MagpieResourcePlan,
    MagpieSamplerResourceBinding, MagpieTextureResourceBinding,
};
use borderless_upscale_core::{UpscaleError, UpscaleResult};

pub const MAGPIE_WGPU_CONSTANT_BINDING_BASE: u32 = 0;
pub const MAGPIE_WGPU_SHADER_RESOURCE_BINDING_BASE: u32 = 32;
pub const MAGPIE_WGPU_UNORDERED_ACCESS_BINDING_BASE: u32 = 64;
pub const MAGPIE_WGPU_SAMPLER_BINDING_BASE: u32 = 96;

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MagpieWgpuBindingLayoutPlan {
    pub passes: Vec<MagpieWgpuPassLayout>,
}

impl MagpieWgpuBindingLayoutPlan {
    pub fn from_resource_plan(resources: &MagpieResourcePlan) -> UpscaleResult<Self> {
        let passes = resources
            .passes
            .iter()
            .map(|pass| wgpu_pass_layout(resources, pass))
            .collect::<UpscaleResult<Vec<_>>>()?;

        Ok(Self { passes })
    }

    #[must_use]
    pub fn pass(&self, pass_index: u32) -> Option<&MagpieWgpuPassLayout> {
        self.passes
            .iter()
            .find(|pass| pass.pass_index == pass_index)
    }

    #[must_use]
    pub fn create_pass_layout_objects(
        &self,
        device: &wgpu::Device,
    ) -> Vec<MagpieWgpuPassLayoutObjects> {
        self.passes
            .iter()
            .map(|pass| pass.create_layout_objects(device))
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MagpieWgpuPassLayout {
    pub pass_index: u32,
    pub pass_name: String,
    pub bindings: Vec<MagpieWgpuBinding>,
    pub entries: Vec<wgpu::BindGroupLayoutEntry>,
}

impl MagpieWgpuPassLayout {
    #[must_use]
    pub fn bind_group_layout_descriptor(&self) -> wgpu::BindGroupLayoutDescriptor<'_> {
        wgpu::BindGroupLayoutDescriptor {
            label: Some(self.pass_name.as_str()),
            entries: &self.entries,
        }
    }

    #[must_use]
    pub fn create_bind_group_layout(&self, device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&self.bind_group_layout_descriptor())
    }

    #[must_use]
    pub fn create_layout_objects(&self, device: &wgpu::Device) -> MagpieWgpuPassLayoutObjects {
        let bind_group_layout = self.create_bind_group_layout(device);
        let bind_group_layouts = [Some(&bind_group_layout)];
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(self.pass_name.as_str()),
            bind_group_layouts: &bind_group_layouts,
            immediate_size: 0,
        });

        MagpieWgpuPassLayoutObjects {
            pass_index: self.pass_index,
            pass_name: self.pass_name.clone(),
            bind_group_layout,
            pipeline_layout,
        }
    }
}

#[derive(Clone, Debug)]
pub struct MagpieWgpuPassLayoutObjects {
    pub pass_index: u32,
    pub pass_name: String,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub pipeline_layout: wgpu::PipelineLayout,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MagpieWgpuBinding {
    pub name: String,
    pub register: u32,
    pub binding: u32,
    pub kind: MagpieWgpuBindingKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MagpieWgpuBindingKind {
    ConstantBuffer,
    ShaderResource,
    UnorderedAccess,
    Sampler,
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

fn wgpu_pass_layout(
    resources: &MagpieResourcePlan,
    pass: &MagpieResourcePass,
) -> UpscaleResult<MagpieWgpuPassLayout> {
    let constant_buffers = std::iter::once(&resources.constant_buffer)
        .chain(resources.dynamic_constant_buffer.iter())
        .map(wgpu_constant_buffer_binding);
    let shader_resources = pass
        .shader_resources
        .iter()
        .map(wgpu_shader_resource_binding);
    let unordered_access_views = pass
        .unordered_access_views
        .iter()
        .map(wgpu_unordered_access_binding);
    let samplers = pass.samplers.iter().map(wgpu_sampler_binding);
    let bindings = constant_buffers
        .chain(shader_resources)
        .chain(unordered_access_views)
        .chain(samplers)
        .collect::<UpscaleResult<Vec<_>>>()?;
    let entries = bindings
        .iter()
        .map(|binding| wgpu_bind_group_layout_entry(resources, pass, binding))
        .collect::<UpscaleResult<Vec<_>>>()?;

    Ok(MagpieWgpuPassLayout {
        pass_index: pass.pass_index,
        pass_name: pass.pass_name.clone(),
        bindings,
        entries,
    })
}

fn wgpu_constant_buffer_binding(
    binding: &MagpieConstantBufferBinding,
) -> UpscaleResult<MagpieWgpuBinding> {
    Ok(MagpieWgpuBinding {
        name: binding.name.clone(),
        register: binding.register,
        binding: binding_index(MAGPIE_WGPU_CONSTANT_BINDING_BASE, binding.register)?,
        kind: MagpieWgpuBindingKind::ConstantBuffer,
    })
}

fn wgpu_shader_resource_binding(
    binding: &MagpieTextureResourceBinding,
) -> UpscaleResult<MagpieWgpuBinding> {
    Ok(MagpieWgpuBinding {
        name: binding.name.clone(),
        register: binding.register,
        binding: binding_index(MAGPIE_WGPU_SHADER_RESOURCE_BINDING_BASE, binding.register)?,
        kind: MagpieWgpuBindingKind::ShaderResource,
    })
}

fn wgpu_unordered_access_binding(
    binding: &MagpieTextureResourceBinding,
) -> UpscaleResult<MagpieWgpuBinding> {
    Ok(MagpieWgpuBinding {
        name: binding.name.clone(),
        register: binding.register,
        binding: binding_index(MAGPIE_WGPU_UNORDERED_ACCESS_BINDING_BASE, binding.register)?,
        kind: MagpieWgpuBindingKind::UnorderedAccess,
    })
}

fn wgpu_sampler_binding(
    binding: &MagpieSamplerResourceBinding,
) -> UpscaleResult<MagpieWgpuBinding> {
    Ok(MagpieWgpuBinding {
        name: binding.name.clone(),
        register: binding.register,
        binding: binding_index(MAGPIE_WGPU_SAMPLER_BINDING_BASE, binding.register)?,
        kind: MagpieWgpuBindingKind::Sampler,
    })
}

fn binding_index(base: u32, register: u32) -> UpscaleResult<u32> {
    base.checked_add(register)
        .ok_or_else(|| invalid_pipeline("Magpie wgpu binding index overflowed"))
}

fn wgpu_bind_group_layout_entry(
    resources: &MagpieResourcePlan,
    pass: &MagpieResourcePass,
    binding: &MagpieWgpuBinding,
) -> UpscaleResult<wgpu::BindGroupLayoutEntry> {
    Ok(wgpu::BindGroupLayoutEntry {
        binding: binding.binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu_binding_type(resources, pass, binding)?,
        count: None,
    })
}

fn wgpu_binding_type(
    resources: &MagpieResourcePlan,
    pass: &MagpieResourcePass,
    binding: &MagpieWgpuBinding,
) -> UpscaleResult<wgpu::BindingType> {
    match binding.kind {
        MagpieWgpuBindingKind::ConstantBuffer => {
            let buffer = constant_buffer_by_name(resources, &binding.name)?;
            Ok(wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(buffer.byte_len as u64),
            })
        }
        MagpieWgpuBindingKind::ShaderResource => {
            let texture = shader_resource_by_name(pass, &binding.name)?;
            Ok(wgpu::BindingType::Texture {
                sample_type: wgpu_texture_sample_type(&texture.format),
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            })
        }
        MagpieWgpuBindingKind::UnorderedAccess => {
            let texture = unordered_access_by_name(pass, &binding.name)?;
            Ok(wgpu::BindingType::StorageTexture {
                access: wgpu::StorageTextureAccess::WriteOnly,
                format: wgpu_texture_format(&texture.format)?,
                view_dimension: wgpu::TextureViewDimension::D2,
            })
        }
        MagpieWgpuBindingKind::Sampler => {
            let sampler = sampler_by_name(pass, &binding.name)?;
            Ok(wgpu::BindingType::Sampler(wgpu_sampler_binding_type(
                sampler.filter,
            )))
        }
    }
}

fn constant_buffer_by_name<'a>(
    resources: &'a MagpieResourcePlan,
    name: &str,
) -> UpscaleResult<&'a MagpieConstantBufferBinding> {
    std::iter::once(&resources.constant_buffer)
        .chain(resources.dynamic_constant_buffer.iter())
        .find(|buffer| buffer.name == name)
        .ok_or_else(|| invalid_pipeline(format!("missing Magpie constant buffer {name}")))
}

fn shader_resource_by_name<'a>(
    pass: &'a MagpieResourcePass,
    name: &str,
) -> UpscaleResult<&'a MagpieTextureResourceBinding> {
    pass.shader_resources
        .iter()
        .find(|texture| texture.name == name)
        .ok_or_else(|| invalid_pipeline(format!("missing Magpie shader resource {name}")))
}

fn unordered_access_by_name<'a>(
    pass: &'a MagpieResourcePass,
    name: &str,
) -> UpscaleResult<&'a MagpieTextureResourceBinding> {
    pass.unordered_access_views
        .iter()
        .find(|texture| texture.name == name)
        .ok_or_else(|| invalid_pipeline(format!("missing Magpie unordered access view {name}")))
}

fn sampler_by_name<'a>(
    pass: &'a MagpieResourcePass,
    name: &str,
) -> UpscaleResult<&'a MagpieSamplerResourceBinding> {
    pass.samplers
        .iter()
        .find(|sampler| sampler.name == name)
        .ok_or_else(|| invalid_pipeline(format!("missing Magpie sampler {name}")))
}

fn wgpu_texture_sample_type(format: &MagpieTextureFormat) -> wgpu::TextureSampleType {
    match format.descriptor().component {
        MagpieTextureComponent::Float
        | MagpieTextureComponent::Unorm
        | MagpieTextureComponent::Snorm
        | MagpieTextureComponent::Unknown => wgpu::TextureSampleType::Float {
            filterable: wgpu_filterable_texture(format),
        },
    }
}

fn wgpu_filterable_texture(format: &MagpieTextureFormat) -> bool {
    matches!(
        format,
        MagpieTextureFormat::R8Unorm
            | MagpieTextureFormat::R8g8Unorm
            | MagpieTextureFormat::R8g8b8a8Unorm
            | MagpieTextureFormat::R8g8b8a8UnormSrgb
            | MagpieTextureFormat::R8g8b8a8Snorm
            | MagpieTextureFormat::R16Float
            | MagpieTextureFormat::R16g16Float
            | MagpieTextureFormat::R16g16b16a16Float
    )
}

fn wgpu_sampler_binding_type(filter: MagpieFxSamplerFilter) -> wgpu::SamplerBindingType {
    match filter {
        MagpieFxSamplerFilter::Point => wgpu::SamplerBindingType::NonFiltering,
        MagpieFxSamplerFilter::Linear => wgpu::SamplerBindingType::Filtering,
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
    use crate::package::MagpieEffectPackage;
    use crate::plan::MagpieRenderPlan;
    use crate::plan::MagpieTextureFormat;
    use crate::resources::MagpieResourcePlan;
    use borderless_upscale_core::FrameSize;
    use std::path::PathBuf;

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

    #[test]
    fn maps_pass_resources_to_wgpu_bind_group_layout_entries() {
        let package = package(
            r"
//!MAGPIE EFFECT
//!VERSION 4
//!USE _DYNAMIC
//!TEXTURE
Texture2D INPUT;
//!TEXTURE
//!WIDTH INPUT_WIDTH
//!HEIGHT INPUT_HEIGHT
Texture2D tex1;
//!TEXTURE
Texture2D OUTPUT;
//!SAMPLER
//!FILTER LINEAR
//!ADDRESS CLAMP
SamplerState LINEAR;
//!PASS 1
//!STYLE PS
//!IN INPUT
//!OUT tex1
//!DESC Copy
MF4 Pass1(float2 pos) { return INPUT.Sample(LINEAR, pos); }
//!PASS 2
//!IN tex1
//!OUT OUTPUT
//!BLOCK_SIZE 8
//!NUM_THREADS 64
void Pass2(uint2 pos) { OUTPUT[pos] = tex1[pos]; }
",
        );
        let resources = MagpieResourcePlan::from_package(&package).unwrap();

        let plan = MagpieWgpuBindingLayoutPlan::from_resource_plan(&resources).unwrap();
        let pass = plan.pass(1).unwrap();

        assert_eq!(pass.pass_name, "Copy");
        assert_eq!(pass.bindings.len(), 5);
        assert!(pass.entries.iter().any(|entry| {
            entry.binding == MAGPIE_WGPU_CONSTANT_BINDING_BASE
                && matches!(
                    entry.ty,
                    wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        ..
                    }
                )
        }));
        assert!(pass.entries.iter().any(|entry| {
            entry.binding == MAGPIE_WGPU_SHADER_RESOURCE_BINDING_BASE
                && matches!(entry.ty, wgpu::BindingType::Texture { .. })
        }));
        assert!(pass.entries.iter().any(|entry| {
            entry.binding == MAGPIE_WGPU_UNORDERED_ACCESS_BINDING_BASE
                && matches!(entry.ty, wgpu::BindingType::StorageTexture { .. })
        }));
        assert!(pass.entries.iter().any(|entry| {
            entry.binding == MAGPIE_WGPU_SAMPLER_BINDING_BASE
                && matches!(
                    entry.ty,
                    wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)
                )
        }));

        let descriptor = pass.bind_group_layout_descriptor();
        assert_eq!(descriptor.label, Some("Copy"));
        assert_eq!(descriptor.entries.len(), pass.entries.len());
        assert_eq!(descriptor.entries[0].binding, pass.entries[0].binding);
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

    fn package(source: &str) -> MagpieEffectPackage {
        let effect = crate::magpiefx::parse_magpiefx(source).unwrap();
        let input_size = FrameSize::new(320, 240).unwrap();
        let output_size = FrameSize::new(640, 480).unwrap();
        let render_plan = MagpieRenderPlan::from_effect(&effect, input_size, output_size).unwrap();
        MagpieEffectPackage {
            effect_path: PathBuf::from("test.hlsl"),
            compiler_source: effect.hlsl_source.clone(),
            compiler_prelude_source: effect.prelude_source.clone(),
            compiler_common_source: effect.common_source.clone(),
            compiler_pass_sources: effect
                .passes
                .iter()
                .map(|pass| pass.source.clone())
                .collect(),
            effect,
            render_plan,
            includes: Vec::new(),
            source_assets: Vec::new(),
        }
    }
}
