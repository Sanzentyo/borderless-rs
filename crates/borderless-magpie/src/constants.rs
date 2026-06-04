// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: Copyright (c) Xu and Magpie contributors
// SPDX-FileCopyrightText: Copyright (c) Borderless Oxide contributors
// Magpie-compatible constant buffer planning for the GPL port:
// https://github.com/Blinue/Magpie

use crate::magpiefx::{MagpieFxParameter, MagpieFxParameterType, MagpieFxPassStyle};
use crate::package::MagpieEffectPackage;
use crate::plan::{MagpiePassPlan, MagpieRenderPlan};
use borderless_upscale_core::{FrameSize, UpscaleError, UpscaleResult};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

const BUILTIN_CONSTANT_DWORDS: usize = 10;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieConstantBufferPlan {
    pub dwords: Vec<u32>,
    pub entries: Vec<MagpieConstantEntry>,
}

impl MagpieConstantBufferPlan {
    pub fn from_package(
        package: &MagpieEffectPackage,
        options: &MagpieConstantBufferOptions,
    ) -> UpscaleResult<Self> {
        Self::from_parts(&package.render_plan, &package.effect.parameters, options)
    }

    pub fn from_parts(
        render_plan: &MagpieRenderPlan,
        parameters: &[MagpieFxParameter],
        options: &MagpieConstantBufferOptions,
    ) -> UpscaleResult<Self> {
        validate_parameter_overrides(parameters, &options.parameter_overrides)?;
        let mut dwords = Vec::with_capacity(BUILTIN_CONSTANT_DWORDS);
        let mut entries = Vec::new();
        append_builtin_constants(
            &mut dwords,
            &mut entries,
            render_plan.input_size,
            render_plan.output_size,
        );
        append_ps_style_pass_constants(&mut dwords, &mut entries, render_plan)?;
        if !options.inline_parameters {
            append_parameter_constants(
                &mut dwords,
                &mut entries,
                parameters,
                &options.parameter_overrides,
            )?;
        }
        pad_to_16_bytes(&mut dwords);
        Ok(Self { dwords, entries })
    }

    #[must_use]
    pub fn byte_len(&self) -> usize {
        self.dwords.len() * std::mem::size_of::<u32>()
    }

    #[must_use]
    pub fn to_le_bytes(&self) -> Vec<u8> {
        self.dwords
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieConstantBufferOptions {
    pub inline_parameters: bool,
    pub parameter_overrides: Vec<MagpieParameterValue>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MagpieParameterValue {
    pub name: String,
    pub value: f32,
}

impl Eq for MagpieParameterValue {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieConstantEntry {
    pub name: String,
    pub offset_dwords: usize,
    pub value: MagpieConstantValue,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MagpieConstantValue {
    Uint { value: u32 },
    Float { bits: u32 },
    Int { value: i32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieDynamicConstantBuffer {
    pub frame_count: u32,
}

impl MagpieDynamicConstantBuffer {
    #[must_use]
    pub const fn dwords(self) -> [u32; 4] {
        [self.frame_count, 0, 0, 0]
    }

    #[must_use]
    pub fn to_le_bytes(self) -> Vec<u8> {
        self.dwords()
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect()
    }
}

fn append_builtin_constants(
    dwords: &mut Vec<u32>,
    entries: &mut Vec<MagpieConstantEntry>,
    input_size: FrameSize,
    output_size: FrameSize,
) {
    push_uint(dwords, entries, "__inputSize.x", input_size.width);
    push_uint(dwords, entries, "__inputSize.y", input_size.height);
    push_uint(dwords, entries, "__outputSize.x", output_size.width);
    push_uint(dwords, entries, "__outputSize.y", output_size.height);
    push_float(dwords, entries, "__inputPt.x", reciprocal(input_size.width));
    push_float(
        dwords,
        entries,
        "__inputPt.y",
        reciprocal(input_size.height),
    );
    push_float(
        dwords,
        entries,
        "__outputPt.x",
        reciprocal(output_size.width),
    );
    push_float(
        dwords,
        entries,
        "__outputPt.y",
        reciprocal(output_size.height),
    );
    push_float(
        dwords,
        entries,
        "__scale.x",
        f32_from_dimension(output_size.width) / f32_from_dimension(input_size.width),
    );
    push_float(
        dwords,
        entries,
        "__scale.y",
        f32_from_dimension(output_size.height) / f32_from_dimension(input_size.height),
    );
}

fn append_ps_style_pass_constants(
    dwords: &mut Vec<u32>,
    entries: &mut Vec<MagpieConstantEntry>,
    render_plan: &MagpieRenderPlan,
) -> UpscaleResult<()> {
    for (index, pass) in render_plan
        .passes
        .iter()
        .enumerate()
        .take(render_plan.passes.len().saturating_sub(1))
    {
        if pass.style == MagpieFxPassStyle::PixelShader {
            let pass_index = index + 1;
            let size = first_output_size(pass, render_plan)?;
            push_uint(
                dwords,
                entries,
                format!("__pass{pass_index}OutputSize.x"),
                size.width,
            );
            push_uint(
                dwords,
                entries,
                format!("__pass{pass_index}OutputSize.y"),
                size.height,
            );
            push_float(
                dwords,
                entries,
                format!("__pass{pass_index}OutputPt.x"),
                reciprocal(size.width),
            );
            push_float(
                dwords,
                entries,
                format!("__pass{pass_index}OutputPt.y"),
                reciprocal(size.height),
            );
        }
    }
    Ok(())
}

fn append_parameter_constants(
    dwords: &mut Vec<u32>,
    entries: &mut Vec<MagpieConstantEntry>,
    parameters: &[MagpieFxParameter],
    overrides: &[MagpieParameterValue],
) -> UpscaleResult<()> {
    parameters.iter().try_for_each(|parameter| {
        let value = overrides
            .iter()
            .find(|override_value| override_value.name == parameter.symbol)
            .map_or_else(
                || default_parameter_value(parameter),
                |override_value| Ok(override_value.value),
            )?;
        match parameter.value_type {
            MagpieFxParameterType::Float => {
                validate_float_parameter(parameter, value)?;
                push_float(dwords, entries, &parameter.symbol, value);
            }
            MagpieFxParameterType::Int => {
                let value = rounded_i32(value)?;
                validate_int_parameter(parameter, value)?;
                push_int(dwords, entries, &parameter.symbol, value);
            }
            MagpieFxParameterType::Unknown => {
                return Err(invalid_pipeline(format!(
                    "Magpie parameter {} has unsupported type",
                    parameter.symbol
                )));
            }
        }
        Ok(())
    })
}

fn first_output_size(
    pass: &MagpiePassPlan,
    render_plan: &MagpieRenderPlan,
) -> UpscaleResult<FrameSize> {
    let output = pass
        .outputs
        .first()
        .ok_or_else(|| invalid_pipeline(format!("pass {} has no output texture", pass.name)))?;
    render_plan
        .texture(output)
        .and_then(|texture| texture.size)
        .ok_or_else(|| invalid_pipeline(format!("output texture {output} has unresolved size")))
}

fn validate_parameter_overrides(
    parameters: &[MagpieFxParameter],
    overrides: &[MagpieParameterValue],
) -> UpscaleResult<()> {
    let parameter_names = parameters
        .iter()
        .map(|parameter| parameter.symbol.as_str())
        .collect::<HashSet<_>>();
    overrides.iter().try_for_each(|override_value| {
        parameter_names
            .contains(override_value.name.as_str())
            .then_some(())
            .ok_or_else(|| {
                invalid_pipeline(format!(
                    "constant override references unknown Magpie parameter {}",
                    override_value.name
                ))
            })
    })
}

fn default_parameter_value(parameter: &MagpieFxParameter) -> UpscaleResult<f32> {
    parameter
        .default_value
        .as_deref()
        .ok_or_else(|| {
            invalid_pipeline(format!("parameter {} is missing DEFAULT", parameter.symbol))
        })
        .and_then(parse_f32)
}

fn validate_float_parameter(parameter: &MagpieFxParameter, value: f32) -> UpscaleResult<()> {
    let min = parameter.min.as_deref().map(parse_f32).transpose()?;
    let max = parameter.max.as_deref().map(parse_f32).transpose()?;
    if min.is_some_and(|min| value < min) || max.is_some_and(|max| value > max) {
        return Err(invalid_pipeline(format!(
            "parameter {} value {value} is outside the allowed range",
            parameter.symbol
        )));
    }
    Ok(())
}

fn validate_int_parameter(parameter: &MagpieFxParameter, value: i32) -> UpscaleResult<()> {
    let min = parameter.min.as_deref().map(parse_i32).transpose()?;
    let max = parameter.max.as_deref().map(parse_i32).transpose()?;
    if min.is_some_and(|min| value < min) || max.is_some_and(|max| value > max) {
        return Err(invalid_pipeline(format!(
            "parameter {} value {value} is outside the allowed range",
            parameter.symbol
        )));
    }
    Ok(())
}

fn parse_f32(value: &str) -> UpscaleResult<f32> {
    value
        .trim()
        .trim_end_matches(['f', 'F'])
        .parse::<f32>()
        .map_err(|_| invalid_pipeline(format!("invalid Magpie float constant: {value}")))
}

fn parse_i32(value: &str) -> UpscaleResult<i32> {
    value
        .trim()
        .parse::<i32>()
        .map_err(|_| invalid_pipeline(format!("invalid Magpie integer constant: {value}")))
}

fn push_uint(
    dwords: &mut Vec<u32>,
    entries: &mut Vec<MagpieConstantEntry>,
    name: impl Into<String>,
    value: u32,
) {
    let offset_dwords = dwords.len();
    dwords.push(value);
    entries.push(MagpieConstantEntry {
        name: name.into(),
        offset_dwords,
        value: MagpieConstantValue::Uint { value },
    });
}

fn push_float(
    dwords: &mut Vec<u32>,
    entries: &mut Vec<MagpieConstantEntry>,
    name: impl Into<String>,
    value: f32,
) {
    let offset_dwords = dwords.len();
    let bits = value.to_bits();
    dwords.push(bits);
    entries.push(MagpieConstantEntry {
        name: name.into(),
        offset_dwords,
        value: MagpieConstantValue::Float { bits },
    });
}

fn push_int(
    dwords: &mut Vec<u32>,
    entries: &mut Vec<MagpieConstantEntry>,
    name: impl Into<String>,
    value: i32,
) {
    let offset_dwords = dwords.len();
    dwords.push(u32::from_ne_bytes(value.to_ne_bytes()));
    entries.push(MagpieConstantEntry {
        name: name.into(),
        offset_dwords,
        value: MagpieConstantValue::Int { value },
    });
}

fn pad_to_16_bytes(dwords: &mut Vec<u32>) {
    dwords.resize(dwords.len().next_multiple_of(4), 0);
}

fn reciprocal(value: u32) -> f32 {
    1.0 / f32_from_dimension(value)
}

#[allow(clippy::cast_precision_loss)]
fn f32_from_dimension(value: u32) -> f32 {
    value as f32
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn rounded_i32(value: f32) -> UpscaleResult<i32> {
    let rounded = value.round();
    if !rounded.is_finite() || rounded < i32::MIN as f32 || rounded > i32::MAX as f32 {
        return Err(invalid_pipeline(format!(
            "Magpie integer parameter value {value} cannot be represented as i32"
        )));
    }
    Ok(rounded as i32)
}

fn invalid_pipeline(message: impl Into<String>) -> UpscaleError {
    UpscaleError::InvalidPipeline(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::magpiefx::parse_magpiefx;

    #[test]
    fn builds_magpie_cb1_builtin_and_parameter_dwords() {
        let package = package(
            r"
//!MAGPIE EFFECT
//!VERSION 4
//!PARAMETER
//!LABEL Sharpness
//!DEFAULT 1
//!MIN 0
//!MAX 2
//!STEP 0.1
float sharpness;
//!PARAMETER
//!LABEL Mode
//!DEFAULT 1
//!MIN 0
//!MAX 3
//!STEP 1
int mode;
//!TEXTURE
Texture2D INPUT;
//!TEXTURE
Texture2D OUTPUT;
//!PASS 1
//!IN INPUT
//!OUT OUTPUT
//!BLOCK_SIZE 8
//!NUM_THREADS 64
void Pass1(uint2 blockStart, uint3 threadId) {}
",
            FrameSize::new(640, 480).unwrap(),
            FrameSize::new(1280, 960).unwrap(),
        );

        let plan = MagpieConstantBufferPlan::from_package(
            &package,
            &MagpieConstantBufferOptions {
                inline_parameters: false,
                parameter_overrides: vec![
                    MagpieParameterValue {
                        name: "sharpness".to_owned(),
                        value: 1.5,
                    },
                    MagpieParameterValue {
                        name: "mode".to_owned(),
                        value: 2.4,
                    },
                ],
            },
        )
        .unwrap();

        assert_eq!(plan.dwords[0], 640);
        assert_eq!(plan.dwords[1], 480);
        assert_eq!(plan.dwords[2], 1280);
        assert_eq!(plan.dwords[3], 960);
        assert_eq!(plan.dwords[8], 2.0f32.to_bits());
        assert_eq!(plan.dwords[9], 2.0f32.to_bits());
        assert_eq!(plan.dwords[10], 1.5f32.to_bits());
        assert_eq!(plan.dwords[11], 2);
        assert_eq!(plan.dwords.len() % 4, 0);
        assert_eq!(plan.byte_len(), 48);
    }

    #[test]
    fn includes_non_final_ps_style_pass_output_size() {
        let package = package(
            r"
//!MAGPIE EFFECT
//!VERSION 4
//!TEXTURE
Texture2D INPUT;
//!TEXTURE
Texture2D OUTPUT;
//!TEXTURE
//!WIDTH INPUT_WIDTH
//!HEIGHT INPUT_HEIGHT
Texture2D tex1;
//!PASS 1
//!STYLE PS
//!IN INPUT
//!OUT tex1
float4 Pass1(float2 pos) { return 1; }
//!PASS 2
//!IN tex1
//!OUT OUTPUT
//!BLOCK_SIZE 16
//!NUM_THREADS 64
void Pass2(uint2 blockStart, uint3 threadId) {}
",
            FrameSize::new(640, 480).unwrap(),
            FrameSize::new(1280, 960).unwrap(),
        );

        let plan = MagpieConstantBufferPlan::from_package(
            &package,
            &MagpieConstantBufferOptions::default(),
        )
        .unwrap();

        assert_eq!(plan.dwords[10], 640);
        assert_eq!(plan.dwords[11], 480);
        assert_eq!(plan.dwords[12], (1.0f32 / 640.0).to_bits());
        assert_eq!(plan.dwords[13], (1.0f32 / 480.0).to_bits());
    }

    #[test]
    fn omits_parameters_when_inline_parameters_are_enabled() {
        let package = package(
            r"
//!MAGPIE EFFECT
//!VERSION 4
//!PARAMETER
//!LABEL Sharpness
//!DEFAULT 1
//!MIN 0
//!MAX 2
//!STEP 0.1
float sharpness;
//!TEXTURE
Texture2D INPUT;
//!TEXTURE
Texture2D OUTPUT;
//!PASS 1
//!IN INPUT
//!OUT OUTPUT
//!BLOCK_SIZE 8
//!NUM_THREADS 64
void Pass1(uint2 blockStart, uint3 threadId) {}
",
            FrameSize::new(640, 480).unwrap(),
            FrameSize::new(1280, 960).unwrap(),
        );

        let plan = MagpieConstantBufferPlan::from_package(
            &package,
            &MagpieConstantBufferOptions {
                inline_parameters: true,
                parameter_overrides: Vec::new(),
            },
        )
        .unwrap();

        assert_eq!(
            plan.dwords.len(),
            BUILTIN_CONSTANT_DWORDS.next_multiple_of(4)
        );
        assert!(!plan.entries.iter().any(|entry| entry.name == "sharpness"));
    }

    fn package(source: &str, input_size: FrameSize, output_size: FrameSize) -> MagpieEffectPackage {
        let effect = parse_magpiefx(source).unwrap();
        let render_plan = MagpieRenderPlan::from_effect(&effect, input_size, output_size).unwrap();
        MagpieEffectPackage {
            effect_path: "effect.hlsl".into(),
            compiler_source: effect.hlsl_source.clone(),
            compiler_prelude_source: effect.prelude_source.clone(),
            compiler_common_source: effect.common_source.clone(),
            compiler_pass_sources: effect
                .passes
                .iter()
                .map(|pass| pass.source.clone())
                .collect(),
            includes: Vec::new(),
            source_assets: Vec::new(),
            effect,
            render_plan,
        }
    }
}
