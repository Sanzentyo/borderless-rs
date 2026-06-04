// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: Copyright (c) Xu and Magpie contributors
// SPDX-FileCopyrightText: Copyright (c) Borderless Oxide contributors
// Magpie-compatible renderer execution planning for the GPL port:
// https://github.com/Blinue/Magpie

use crate::compiled::MagpieCompiledEffect;
use crate::hlsl_compiler::MagpieCompiledShader;
use crate::resources::{
    MagpieConstantBufferBinding, MagpieResourcePass, MagpieSamplerResourceBinding,
    MagpieTextureResourceBinding,
};
use borderless_upscale_core::{UpscaleError, UpscaleResult};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieExecutionPlan {
    pub source_name: String,
    pub commands: Vec<MagpieExecutionCommand>,
}

impl MagpieExecutionPlan {
    pub fn from_compiled_effect(effect: &MagpieCompiledEffect) -> UpscaleResult<Self> {
        let commands = effect
            .resources
            .passes
            .iter()
            .zip(effect.shaders.iter())
            .map(|(pass, shader)| pass_commands(pass, shader, effect))
            .collect::<UpscaleResult<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect();

        Ok(Self {
            source_name: effect.source_name.clone(),
            commands,
        })
    }

    #[must_use]
    pub fn pass_commands(&self, pass_index: u32) -> Vec<&MagpieExecutionCommand> {
        self.commands
            .iter()
            .filter(|command| command.pass_index() == pass_index)
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MagpieExecutionCommand {
    BeginPass {
        pass_index: u32,
        pass_name: String,
    },
    BindComputePipeline {
        pass_index: u32,
        bytecode_len: usize,
        entry_point: String,
        target_profile: String,
    },
    BindConstantBuffer {
        pass_index: u32,
        binding: MagpieConstantBufferBinding,
    },
    BindShaderResource {
        pass_index: u32,
        binding: MagpieTextureResourceBinding,
    },
    BindUnorderedAccess {
        pass_index: u32,
        binding: MagpieTextureResourceBinding,
    },
    BindSampler {
        pass_index: u32,
        binding: MagpieSamplerResourceBinding,
    },
    Dispatch {
        pass_index: u32,
        group_count: [u32; 3],
        block_size: [u32; 2],
    },
    EndPass {
        pass_index: u32,
    },
}

impl MagpieExecutionCommand {
    #[must_use]
    pub const fn pass_index(&self) -> u32 {
        match self {
            Self::BeginPass { pass_index, .. }
            | Self::BindComputePipeline { pass_index, .. }
            | Self::BindConstantBuffer { pass_index, .. }
            | Self::BindShaderResource { pass_index, .. }
            | Self::BindUnorderedAccess { pass_index, .. }
            | Self::BindSampler { pass_index, .. }
            | Self::Dispatch { pass_index, .. }
            | Self::EndPass { pass_index } => *pass_index,
        }
    }
}

fn pass_commands(
    pass: &MagpieResourcePass,
    shader: &MagpieCompiledShader,
    effect: &MagpieCompiledEffect,
) -> UpscaleResult<Vec<MagpieExecutionCommand>> {
    validate_pass_shader(pass, shader)?;
    let pass_index = pass.pass_index;
    let mut commands = vec![
        MagpieExecutionCommand::BeginPass {
            pass_index,
            pass_name: pass.pass_name.clone(),
        },
        MagpieExecutionCommand::BindComputePipeline {
            pass_index,
            bytecode_len: shader.bytecode.len(),
            entry_point: shader.entry_point.clone(),
            target_profile: shader.target_profile.clone(),
        },
        MagpieExecutionCommand::BindConstantBuffer {
            pass_index,
            binding: effect.resources.constant_buffer.clone(),
        },
    ];

    if let Some(binding) = &effect.resources.dynamic_constant_buffer {
        commands.push(MagpieExecutionCommand::BindConstantBuffer {
            pass_index,
            binding: binding.clone(),
        });
    }

    commands.extend(pass.shader_resources.iter().cloned().map(|binding| {
        MagpieExecutionCommand::BindShaderResource {
            pass_index,
            binding,
        }
    }));
    commands.extend(pass.unordered_access_views.iter().cloned().map(|binding| {
        MagpieExecutionCommand::BindUnorderedAccess {
            pass_index,
            binding,
        }
    }));
    commands.extend(pass.samplers.iter().cloned().map(|binding| {
        MagpieExecutionCommand::BindSampler {
            pass_index,
            binding,
        }
    }));
    commands.push(MagpieExecutionCommand::Dispatch {
        pass_index,
        group_count: pass.dispatch.group_count,
        block_size: pass.dispatch.block_size,
    });
    commands.push(MagpieExecutionCommand::EndPass { pass_index });
    Ok(commands)
}

fn validate_pass_shader(
    pass: &MagpieResourcePass,
    shader: &MagpieCompiledShader,
) -> UpscaleResult<()> {
    if pass.pass_index != shader.pass_index || pass.pass_name != shader.pass_name {
        return Err(invalid_pipeline(format!(
            "compiled shader pass {} does not match resource pass {}",
            shader.pass_index, pass.pass_index
        )));
    }
    Ok(())
}

fn invalid_pipeline(message: impl Into<String>) -> UpscaleError {
    UpscaleError::InvalidPipeline(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiled::MagpieCompiledEffect;
    use crate::hlsl_compiler::MagpieCompiledShader;
    use crate::magpiefx::parse_magpiefx;
    use crate::package::MagpieEffectPackage;
    use crate::plan::MagpieRenderPlan;
    use crate::resources::MagpieResourcePlan;
    use borderless_upscale_core::FrameSize;
    use std::path::PathBuf;

    #[test]
    fn creates_ordered_commands_for_compiled_effect() {
        let package = package(
            r"
//!MAGPIE EFFECT
//!VERSION 4
//!USE _DYNAMIC
//!TEXTURE
Texture2D INPUT;
//!TEXTURE
Texture2D OUTPUT;
//!SAMPLER
SamplerState LINEAR;
//!PASS 1
//!IN INPUT
//!OUT OUTPUT
//!BLOCK_SIZE 8
//!NUM_THREADS 64
void Pass1(uint2 pos) { OUTPUT[pos] = INPUT[pos]; }
",
        );
        let resources = MagpieResourcePlan::from_package(&package).unwrap();
        let effect =
            MagpieCompiledEffect::from_parts(resources, vec![compiled_shader(1, "Pass 1")])
                .unwrap();

        let plan = MagpieExecutionPlan::from_compiled_effect(&effect).unwrap();

        assert_eq!(plan.commands.len(), 9);
        assert!(matches!(
            plan.commands[0],
            MagpieExecutionCommand::BeginPass { pass_index: 1, .. }
        ));
        assert!(matches!(
            plan.commands[1],
            MagpieExecutionCommand::BindComputePipeline {
                pass_index: 1,
                bytecode_len: 4,
                ..
            }
        ));
        assert!(matches!(
            plan.commands[2],
            MagpieExecutionCommand::BindConstantBuffer { pass_index: 1, .. }
        ));
        assert!(matches!(
            plan.commands[3],
            MagpieExecutionCommand::BindConstantBuffer { pass_index: 1, .. }
        ));
        assert!(matches!(
            plan.commands[7],
            MagpieExecutionCommand::Dispatch {
                pass_index: 1,
                group_count: [80, 60, 1],
                block_size: [8, 8],
            }
        ));
        assert!(matches!(
            plan.commands[8],
            MagpieExecutionCommand::EndPass { pass_index: 1 }
        ));
    }

    #[test]
    fn filters_commands_by_pass_index() {
        let package = package(
            r"
//!MAGPIE EFFECT
//!VERSION 4
//!TEXTURE
Texture2D INPUT;
//!TEXTURE
//!WIDTH INPUT_WIDTH
//!HEIGHT INPUT_HEIGHT
Texture2D tex1;
//!TEXTURE
Texture2D OUTPUT;
//!PASS 1
//!STYLE PS
//!IN INPUT
//!OUT tex1
MF4 Pass1(float2 pos) { return INPUT.SampleLevel(SamplerState{}, pos, 0); }
//!PASS 2
//!IN tex1
//!OUT OUTPUT
//!BLOCK_SIZE 8
//!NUM_THREADS 64
void Pass2(uint2 pos) { OUTPUT[pos] = tex1[pos]; }
",
        );
        let resources = MagpieResourcePlan::from_package(&package).unwrap();
        let effect = MagpieCompiledEffect::from_parts(
            resources,
            vec![compiled_shader(1, "Pass 1"), compiled_shader(2, "Pass 2")],
        )
        .unwrap();

        let plan = MagpieExecutionPlan::from_compiled_effect(&effect).unwrap();

        assert!(plan.pass_commands(1).len() > 1);
        assert!(
            plan.pass_commands(1)
                .iter()
                .all(|command| command.pass_index() == 1)
        );
        assert!(
            plan.pass_commands(2)
                .iter()
                .all(|command| command.pass_index() == 2)
        );
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
