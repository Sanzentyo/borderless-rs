// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: Copyright (c) Xu and Magpie contributors
// SPDX-FileCopyrightText: Copyright (c) Borderless Oxide contributors
// Magpie-compatible compiled effect bundling for the GPL port:
// https://github.com/Blinue/Magpie

use crate::compiler::MagpieCompilePlan;
use crate::hlsl_compiler::{MagpieCompiledShader, MagpieExternalHlslCompiler};
use crate::package::MagpieEffectPackage;
use crate::resources::{MagpieResourcePlan, MagpieResourcePlanOptions};
use borderless_upscale_core::{UpscaleError, UpscaleResult};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieCompiledEffect {
    pub source_name: String,
    pub resources: MagpieResourcePlan,
    pub shaders: Vec<MagpieCompiledShader>,
}

impl MagpieCompiledEffect {
    pub fn compile_package(
        package: &MagpieEffectPackage,
        compiler: &MagpieExternalHlslCompiler,
    ) -> UpscaleResult<Self> {
        Self::compile_package_with_options(
            package,
            compiler,
            &MagpieCompiledEffectOptions::default(),
        )
    }

    pub fn compile_package_with_options(
        package: &MagpieEffectPackage,
        compiler: &MagpieExternalHlslCompiler,
        options: &MagpieCompiledEffectOptions,
    ) -> UpscaleResult<Self> {
        let compile_plan =
            MagpieCompilePlan::from_package_with_options(package, &options.resources.compile)?;
        let resources = MagpieResourcePlan::from_package_with_options(package, &options.resources)?;
        let shaders = compiler.compile_plan(&compile_plan.jobs)?;
        Self::from_parts(resources, shaders)
    }

    pub fn from_parts(
        resources: MagpieResourcePlan,
        shaders: Vec<MagpieCompiledShader>,
    ) -> UpscaleResult<Self> {
        validate_shader_sequence(&resources, &shaders)?;
        Ok(Self {
            source_name: resources.source_name.clone(),
            resources,
            shaders,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieCompiledEffectOptions {
    pub resources: MagpieResourcePlanOptions,
}

fn validate_shader_sequence(
    resources: &MagpieResourcePlan,
    shaders: &[MagpieCompiledShader],
) -> UpscaleResult<()> {
    if resources.passes.len() != shaders.len() {
        return Err(invalid_pipeline(format!(
            "compiled shader count {} does not match resource pass count {}",
            shaders.len(),
            resources.passes.len()
        )));
    }
    resources
        .passes
        .iter()
        .zip(shaders)
        .try_for_each(|(pass, shader)| {
            if pass.pass_index != shader.pass_index {
                return Err(invalid_pipeline(format!(
                    "compiled shader pass {} does not match resource pass {}",
                    shader.pass_index, pass.pass_index
                )));
            }
            if pass.pass_name != shader.pass_name {
                return Err(invalid_pipeline(format!(
                    "compiled shader pass {} name does not match resource plan",
                    shader.pass_index
                )));
            }
            if shader.bytecode.is_empty() {
                return Err(invalid_pipeline(format!(
                    "compiled shader pass {} has empty bytecode",
                    shader.pass_index
                )));
            }
            Ok(())
        })
}

fn invalid_pipeline(message: impl Into<String>) -> UpscaleError {
    UpscaleError::InvalidPipeline(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::magpiefx::parse_magpiefx;
    use crate::package::MagpieEffectPackage;
    use crate::plan::MagpieRenderPlan;
    use borderless_upscale_core::FrameSize;
    use std::path::PathBuf;

    #[test]
    fn bundles_resource_plan_and_compiled_shader_bytecode() {
        let package = package(
            r"
//!MAGPIE EFFECT
//!VERSION 4
//!TEXTURE
Texture2D INPUT;
//!TEXTURE
Texture2D OUTPUT;
//!PASS 1
//!IN INPUT
//!OUT OUTPUT
//!DESC Final
//!BLOCK_SIZE 8
//!NUM_THREADS 64
void Pass1(uint2 pos) { OUTPUT[pos] = INPUT[pos]; }
",
        );
        let resources = MagpieResourcePlan::from_package(&package).unwrap();
        let shader = compiled_shader(1, "Final");

        let effect = MagpieCompiledEffect::from_parts(resources, vec![shader]).unwrap();

        assert_eq!(effect.source_name, "test.hlsl");
        assert_eq!(effect.shaders[0].bytecode, vec![1, 2, 3, 4]);
        assert_eq!(effect.resources.passes[0].dispatch.group_count, [80, 60, 1]);
    }

    #[test]
    fn rejects_empty_shader_bytecode() {
        let package = package(
            r"
//!MAGPIE EFFECT
//!VERSION 4
//!TEXTURE
Texture2D INPUT;
//!TEXTURE
Texture2D OUTPUT;
//!PASS 1
//!IN INPUT
//!OUT OUTPUT
//!BLOCK_SIZE 8
//!NUM_THREADS 64
void Pass1(uint2 pos) { OUTPUT[pos] = INPUT[pos]; }
",
        );
        let resources = MagpieResourcePlan::from_package(&package).unwrap();
        let mut shader = compiled_shader(1, "Pass 1");
        shader.bytecode.clear();

        assert!(matches!(
            MagpieCompiledEffect::from_parts(resources, vec![shader]),
            Err(UpscaleError::InvalidPipeline(_))
        ));
    }

    fn compiled_shader(pass_index: u32, pass_name: &str) -> MagpieCompiledShader {
        MagpieCompiledShader {
            pass_index,
            pass_name: pass_name.to_owned(),
            entry_point: "__M".to_owned(),
            target_profile: "cs_5_0".to_owned(),
            bytecode: vec![1, 2, 3, 4],
            diagnostics: String::new(),
        }
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
