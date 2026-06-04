// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: Copyright (c) Xu and Magpie contributors
// SPDX-FileCopyrightText: Copyright (c) Borderless Oxide contributors
// Magpie-compatible backend descriptor planning for the GPL port:
// https://github.com/Blinue/Magpie

use crate::allocation::{MagpieTextureAllocation, MagpieTextureUsage};
use crate::formats::MagpieTextureFormatDescriptor;
use crate::magpiefx::{MagpieFxSamplerAddress, MagpieFxSamplerFilter};
use crate::resources::{
    MagpieConstantBufferBinding, MagpieResourcePlan, MagpieSamplerResourceBinding,
};
use borderless_upscale_core::{FrameSize, UpscaleError, UpscaleResult};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieBackendDescriptorPlan {
    pub source_name: String,
    pub textures: Vec<MagpieBackendTextureDescriptor>,
    pub buffers: Vec<MagpieBackendBufferDescriptor>,
    pub samplers: Vec<MagpieBackendSamplerDescriptor>,
}

impl MagpieBackendDescriptorPlan {
    pub fn from_resource_plan(resources: &MagpieResourcePlan) -> UpscaleResult<Self> {
        let textures = resources
            .textures
            .iter()
            .map(texture_descriptor)
            .collect::<UpscaleResult<Vec<_>>>()?;
        let mut buffers = vec![buffer_descriptor(&resources.constant_buffer)];
        if let Some(dynamic) = &resources.dynamic_constant_buffer {
            buffers.push(buffer_descriptor(dynamic));
        }
        let samplers = dedupe_samplers(resources);

        Ok(Self {
            source_name: resources.source_name.clone(),
            textures,
            buffers,
            samplers,
        })
    }

    #[must_use]
    pub fn texture(&self, name: &str) -> Option<&MagpieBackendTextureDescriptor> {
        self.textures.iter().find(|texture| texture.name == name)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieBackendTextureDescriptor {
    pub name: String,
    pub size: FrameSize,
    pub format: MagpieTextureFormatDescriptor,
    pub dxgi_format: u32,
    pub bind_flags: Vec<MagpieBackendTextureBind>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MagpieBackendTextureBind {
    ExternalInput,
    ExternalOutput,
    ShaderResource,
    UnorderedAccess,
    SourceUpload,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieBackendBufferDescriptor {
    pub name: String,
    pub register: u32,
    pub byte_len: usize,
    pub initial_dwords: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieBackendSamplerDescriptor {
    pub name: String,
    pub register: u32,
    pub filter: MagpieFxSamplerFilter,
    pub address: MagpieFxSamplerAddress,
}

fn texture_descriptor(
    texture: &MagpieTextureAllocation,
) -> UpscaleResult<MagpieBackendTextureDescriptor> {
    let dxgi_format = texture.format_descriptor.dxgi_format.ok_or_else(|| {
        invalid_pipeline(format!(
            "texture {} has no renderer-known DXGI format",
            texture.name
        ))
    })?;
    if texture.format_descriptor.bytes_per_pixel.is_none() {
        return Err(invalid_pipeline(format!(
            "texture {} has no renderer-known byte layout",
            texture.name
        )));
    }

    Ok(MagpieBackendTextureDescriptor {
        name: texture.name.clone(),
        size: texture.size,
        format: texture.format_descriptor.clone(),
        dxgi_format,
        bind_flags: texture.usage.iter().copied().map(texture_bind).collect(),
    })
}

fn texture_bind(usage: MagpieTextureUsage) -> MagpieBackendTextureBind {
    match usage {
        MagpieTextureUsage::ExternalInput => MagpieBackendTextureBind::ExternalInput,
        MagpieTextureUsage::ExternalOutput => MagpieBackendTextureBind::ExternalOutput,
        MagpieTextureUsage::ShaderResource => MagpieBackendTextureBind::ShaderResource,
        MagpieTextureUsage::UnorderedAccess => MagpieBackendTextureBind::UnorderedAccess,
        MagpieTextureUsage::SourceUpload => MagpieBackendTextureBind::SourceUpload,
    }
}

fn buffer_descriptor(binding: &MagpieConstantBufferBinding) -> MagpieBackendBufferDescriptor {
    MagpieBackendBufferDescriptor {
        name: binding.name.clone(),
        register: binding.register,
        byte_len: binding.byte_len,
        initial_dwords: binding.initial_dwords.clone(),
    }
}

fn dedupe_samplers(resources: &MagpieResourcePlan) -> Vec<MagpieBackendSamplerDescriptor> {
    resources
        .passes
        .iter()
        .flat_map(|pass| pass.samplers.iter())
        .fold(Vec::new(), |mut samplers, sampler| {
            if !samplers
                .iter()
                .any(|existing| same_sampler(existing, sampler))
            {
                samplers.push(MagpieBackendSamplerDescriptor {
                    name: sampler.name.clone(),
                    register: sampler.register,
                    filter: sampler.filter,
                    address: sampler.address,
                });
            }
            samplers
        })
}

fn same_sampler(
    existing: &MagpieBackendSamplerDescriptor,
    sampler: &MagpieSamplerResourceBinding,
) -> bool {
    existing.name == sampler.name
        && existing.register == sampler.register
        && existing.filter == sampler.filter
        && existing.address == sampler.address
}

fn invalid_pipeline(message: impl Into<String>) -> UpscaleError {
    UpscaleError::InvalidPipeline(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::magpiefx::{MagpieFxSamplerAddress, MagpieFxSamplerFilter, parse_magpiefx};
    use crate::package::MagpieEffectPackage;
    use crate::plan::MagpieRenderPlan;
    use borderless_upscale_core::FrameSize;
    use std::path::PathBuf;

    #[test]
    fn builds_backend_descriptors_from_resource_plan() {
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
//!FILTER POINT
//!ADDRESS WRAP
SamplerState POINT;
//!PASS 1
//!STYLE PS
//!IN INPUT
//!OUT tex1
MF4 Pass1(float2 pos) { return INPUT.Sample(POINT, pos); }
//!PASS 2
//!IN tex1
//!OUT OUTPUT
//!BLOCK_SIZE 8
//!NUM_THREADS 64
void Pass2(uint2 pos) { OUTPUT[pos] = tex1[pos]; }
",
        );
        let resources = MagpieResourcePlan::from_package(&package).unwrap();

        let descriptors = MagpieBackendDescriptorPlan::from_resource_plan(&resources).unwrap();

        let tex1 = descriptors.texture("tex1").unwrap();
        assert_eq!(tex1.dxgi_format, 28);
        assert!(
            tex1.bind_flags
                .contains(&MagpieBackendTextureBind::ShaderResource)
        );
        assert!(
            tex1.bind_flags
                .contains(&MagpieBackendTextureBind::UnorderedAccess)
        );
        assert_eq!(descriptors.buffers.len(), 2);
        assert_eq!(descriptors.samplers.len(), 1);
        assert_eq!(descriptors.samplers[0].filter, MagpieFxSamplerFilter::Point);
        assert_eq!(
            descriptors.samplers[0].address,
            MagpieFxSamplerAddress::Wrap
        );
    }

    #[test]
    fn rejects_unknown_texture_byte_layout() {
        let package = package(
            r"
//!MAGPIE EFFECT
//!VERSION 4
//!TEXTURE
Texture2D INPUT;
//!TEXTURE
//!WIDTH INPUT_WIDTH
//!HEIGHT INPUT_HEIGHT
//!FORMAT UNKNOWN_FORMAT
Texture2D tex1;
//!TEXTURE
Texture2D OUTPUT;
//!PASS 1
//!IN INPUT
//!OUT tex1
//!BLOCK_SIZE 8
//!NUM_THREADS 64
void Pass1(uint2 pos) { tex1[pos] = INPUT[pos]; }
//!PASS 2
//!IN tex1
//!OUT OUTPUT
//!BLOCK_SIZE 8
//!NUM_THREADS 64
void Pass2(uint2 pos) { OUTPUT[pos] = tex1[pos]; }
",
        );
        let resources = MagpieResourcePlan::from_package(&package).unwrap();

        assert!(matches!(
            MagpieBackendDescriptorPlan::from_resource_plan(&resources),
            Err(UpscaleError::InvalidPipeline(_))
        ));
    }

    fn package(source: &str) -> MagpieEffectPackage {
        let effect = parse_magpiefx(source).unwrap();
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
