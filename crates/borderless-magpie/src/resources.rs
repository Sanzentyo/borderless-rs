// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: Copyright (c) Xu and Magpie contributors
// SPDX-FileCopyrightText: Copyright (c) Borderless Oxide contributors
// Magpie-compatible renderer resource planning for the GPL port:
// https://github.com/Blinue/Magpie

use crate::allocation::{MagpieTextureAllocation, MagpieTextureAllocationPlan};
use crate::compiler::{
    MagpieCompileOptions, MagpieCompilePlan, MagpieShaderJob, MagpieTextureAccess,
    MagpieTextureBinding,
};
use crate::constants::{
    MagpieConstantBufferOptions, MagpieConstantBufferPlan, MagpieDynamicConstantBuffer,
};
use crate::dispatch::{MagpieDispatchPass, MagpieDispatchPlan};
use crate::package::MagpieEffectPackage;
use crate::plan::{MagpieTextureFormat, MagpieTexturePlan, MagpieTextureRole};
use borderless_upscale_core::{FrameSize, UpscaleError, UpscaleResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const MAGPIE_CB1_REGISTER: u32 = 0;
pub const MAGPIE_CB2_REGISTER: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieResourcePlan {
    pub source_name: String,
    pub textures: Vec<MagpieTextureAllocation>,
    pub constant_buffer: MagpieConstantBufferBinding,
    pub dynamic_constant_buffer: Option<MagpieConstantBufferBinding>,
    pub passes: Vec<MagpieResourcePass>,
}

impl MagpieResourcePlan {
    pub fn from_package(package: &MagpieEffectPackage) -> UpscaleResult<Self> {
        Self::from_package_with_options(package, &MagpieResourcePlanOptions::default())
    }

    pub fn from_package_with_options(
        package: &MagpieEffectPackage,
        options: &MagpieResourcePlanOptions,
    ) -> UpscaleResult<Self> {
        validate_options(options)?;
        let compile_plan = MagpieCompilePlan::from_package_with_options(package, &options.compile)?;
        let texture_allocations =
            MagpieTextureAllocationPlan::from_render_plan(&package.render_plan)?;
        let constants = MagpieConstantBufferPlan::from_package(package, &options.constant_buffer)?;
        let dispatch = MagpieDispatchPlan::from_render_plan(&package.render_plan)?;
        let dynamic_constant_buffer = uses_dynamic(&package.effect.uses)
            .then(|| MagpieConstantBufferBinding::dynamic_cb2(options.initial_frame_count));
        let passes = compile_plan
            .jobs
            .iter()
            .zip(dispatch.passes.iter())
            .map(|(job, dispatch)| resource_pass(job, dispatch, package))
            .collect::<UpscaleResult<Vec<_>>>()?;

        Ok(Self {
            source_name: compile_plan.source_name,
            textures: texture_allocations.textures,
            constant_buffer: MagpieConstantBufferBinding::cb1(constants),
            dynamic_constant_buffer,
            passes,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieResourcePlanOptions {
    pub compile: MagpieCompileOptions,
    pub constant_buffer: MagpieConstantBufferOptions,
    pub initial_frame_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieResourcePass {
    pub pass_index: u32,
    pub pass_name: String,
    pub shader_resources: Vec<MagpieTextureResourceBinding>,
    pub unordered_access_views: Vec<MagpieTextureResourceBinding>,
    pub samplers: Vec<MagpieSamplerResourceBinding>,
    pub dispatch: MagpieDispatchPass,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieConstantBufferBinding {
    pub name: String,
    pub register: u32,
    pub byte_len: usize,
    pub initial_dwords: Vec<u32>,
}

impl MagpieConstantBufferBinding {
    #[must_use]
    pub fn cb1(plan: MagpieConstantBufferPlan) -> Self {
        Self {
            name: "__CB1".to_owned(),
            register: MAGPIE_CB1_REGISTER,
            byte_len: plan.byte_len(),
            initial_dwords: plan.dwords,
        }
    }

    #[must_use]
    pub fn dynamic_cb2(frame_count: u32) -> Self {
        let buffer = MagpieDynamicConstantBuffer { frame_count };
        let dwords = buffer.dwords().to_vec();
        Self {
            name: "__CB2".to_owned(),
            register: MAGPIE_CB2_REGISTER,
            byte_len: dwords.len() * std::mem::size_of::<u32>(),
            initial_dwords: dwords,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieTextureResourceBinding {
    pub name: String,
    pub register: u32,
    pub role: MagpieTextureRole,
    pub format: MagpieTextureFormat,
    pub size: Option<FrameSize>,
    pub source: Option<String>,
    pub source_path: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieSamplerResourceBinding {
    pub name: String,
    pub register: u32,
}

fn resource_pass(
    job: &MagpieShaderJob,
    dispatch: &MagpieDispatchPass,
    package: &MagpieEffectPackage,
) -> UpscaleResult<MagpieResourcePass> {
    let shader_resources = texture_resources(
        &job.texture_bindings,
        package,
        MagpieTextureAccess::ShaderResource,
    )?;
    let unordered_access_views = texture_resources(
        &job.texture_bindings,
        package,
        MagpieTextureAccess::UnorderedAccess,
    )?;
    let samplers = job
        .sampler_bindings
        .iter()
        .map(|sampler| MagpieSamplerResourceBinding {
            name: sampler.name.clone(),
            register: sampler.register,
        })
        .collect();

    Ok(MagpieResourcePass {
        pass_index: job.pass_index,
        pass_name: job.pass_name.clone(),
        shader_resources,
        unordered_access_views,
        samplers,
        dispatch: dispatch.clone(),
    })
}

fn texture_resources(
    bindings: &[MagpieTextureBinding],
    package: &MagpieEffectPackage,
    access: MagpieTextureAccess,
) -> UpscaleResult<Vec<MagpieTextureResourceBinding>> {
    bindings
        .iter()
        .filter(|binding| binding.access == access)
        .map(|binding| texture_resource(binding, package))
        .collect()
}

fn texture_resource(
    binding: &MagpieTextureBinding,
    package: &MagpieEffectPackage,
) -> UpscaleResult<MagpieTextureResourceBinding> {
    let texture = package
        .render_plan
        .texture(&binding.name)
        .ok_or_else(|| invalid_pipeline(format!("missing texture {}", binding.name)))?;
    validate_binding_format(binding, texture)?;
    Ok(MagpieTextureResourceBinding {
        name: texture.name.clone(),
        register: binding.register,
        role: texture.role,
        format: texture.format.clone(),
        size: texture.size,
        source: texture.source.clone(),
        source_path: texture.source_path.clone(),
    })
}

fn validate_binding_format(
    binding: &MagpieTextureBinding,
    texture: &MagpieTexturePlan,
) -> UpscaleResult<()> {
    if binding.format != texture.format {
        return Err(invalid_pipeline(format!(
            "texture {} binding format does not match render plan",
            binding.name
        )));
    }
    Ok(())
}

fn validate_options(options: &MagpieResourcePlanOptions) -> UpscaleResult<()> {
    if options.compile.inline_parameters != options.constant_buffer.inline_parameters {
        return Err(invalid_pipeline(
            "compile and constant-buffer inline parameter modes must match",
        ));
    }
    Ok(())
}

fn uses_dynamic(uses: &[String]) -> bool {
    uses.iter()
        .any(|value| matches!(value.to_ascii_uppercase().as_str(), "_DYNAMIC" | "DYNAMIC"))
}

fn invalid_pipeline(message: impl Into<String>) -> UpscaleError {
    UpscaleError::InvalidPipeline(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::magpiefx::parse_magpiefx;
    use crate::plan::MagpieRenderPlan;

    #[test]
    fn plans_cbv_srv_uav_sampler_and_dispatch_resources() {
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
SamplerState POINT;
//!PASS 1
//!STYLE PS
//!IN INPUT
//!OUT tex1
//!DESC Copy
MF4 Pass1(float2 pos) { return INPUT.Sample(POINT, pos); }
//!PASS 2
//!IN tex1
//!OUT OUTPUT
//!BLOCK_SIZE 8
//!NUM_THREADS 8, 8, 1
void Pass2(uint2 pos) { OUTPUT[pos] = tex1[pos]; }
",
        );

        let plan = MagpieResourcePlan::from_package(&package).unwrap();

        assert_eq!(plan.constant_buffer.register, MAGPIE_CB1_REGISTER);
        assert_eq!(
            plan.dynamic_constant_buffer.unwrap().register,
            MAGPIE_CB2_REGISTER
        );
        assert_eq!(plan.passes.len(), 2);
        assert_eq!(plan.passes[0].pass_name, "Copy");
        assert_eq!(plan.passes[0].shader_resources[0].name, "INPUT");
        assert_eq!(plan.passes[0].shader_resources[0].register, 0);
        assert_eq!(plan.passes[0].unordered_access_views[0].name, "tex1");
        assert_eq!(plan.passes[0].unordered_access_views[0].register, 0);
        assert_eq!(plan.passes[0].samplers[0].name, "POINT");
        assert_eq!(plan.passes[0].dispatch.group_count, [20, 15, 1]);
        assert_eq!(plan.passes[1].dispatch.block_size, [8, 8]);
        assert_eq!(plan.passes[1].dispatch.group_count, [80, 60, 1]);
    }

    #[test]
    fn rejects_mismatched_inline_parameter_options() {
        let package = package(
            r"
//!MAGPIE EFFECT
//!VERSION 4
//!PARAMETER
//!DEFAULT 1
float sharpness;
//!TEXTURE
Texture2D INPUT;
//!TEXTURE
Texture2D OUTPUT;
//!PASS 1
//!IN INPUT
//!OUT OUTPUT
void Pass1(uint2 pos) { OUTPUT[pos] = INPUT[pos]; }
",
        );
        let options = MagpieResourcePlanOptions {
            compile: MagpieCompileOptions {
                inline_parameters: true,
                parameter_overrides: Vec::new(),
            },
            constant_buffer: MagpieConstantBufferOptions::default(),
            initial_frame_count: 0,
        };

        assert!(matches!(
            MagpieResourcePlan::from_package_with_options(&package, &options),
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
