// SPDX-License-Identifier: GPL-3.0-or-later
// Magpie-compatible render resource planning based on the GPL-licensed Magpie project:
// https://github.com/Blinue/Magpie

use crate::magpiefx::{MagpieFx, MagpieFxPassStyle, MagpieFxTexture};
use borderless_upscale_core::{FrameSize, UpscaleError, UpscaleResult};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieRenderPlan {
    pub input_size: FrameSize,
    pub output_size: FrameSize,
    pub textures: Vec<MagpieTexturePlan>,
    pub passes: Vec<MagpiePassPlan>,
}

impl MagpieRenderPlan {
    pub fn from_effect(
        effect: &MagpieFx,
        input_size: FrameSize,
        output_size: FrameSize,
    ) -> UpscaleResult<Self> {
        effect.validate()?;
        let context = DimensionContext {
            input_size,
            output_size,
        };
        let textures = effect
            .textures
            .iter()
            .map(|texture| texture_plan(texture, context))
            .collect::<UpscaleResult<Vec<_>>>()?;
        let texture_names = textures
            .iter()
            .map(|texture| texture.name.as_str())
            .collect::<HashSet<_>>();
        let passes = effect
            .passes
            .iter()
            .map(|pass| {
                let inputs = validate_texture_refs(&pass.inputs, &texture_names)?;
                let outputs = validate_texture_refs(&pass.outputs, &texture_names)?;
                Ok(MagpiePassPlan {
                    name: format!("Pass{}", pass.index),
                    description: pass.description.clone(),
                    style: pass.style,
                    inputs,
                    outputs,
                    block_size: pass.block_size.clone(),
                    num_threads: pass.num_threads.clone(),
                })
            })
            .collect::<UpscaleResult<Vec<_>>>()?;

        Ok(Self {
            input_size,
            output_size,
            textures,
            passes,
        })
    }

    #[must_use]
    pub fn texture(&self, name: &str) -> Option<&MagpieTexturePlan> {
        self.textures.iter().find(|texture| texture.name == name)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieTexturePlan {
    pub name: String,
    pub role: MagpieTextureRole,
    pub size: Option<FrameSize>,
    pub format: MagpieTextureFormat,
    pub source: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MagpieTextureRole {
    Input,
    Output,
    Intermediate,
    SourceAsset,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MagpieTextureFormat {
    R8Unorm,
    R8g8Unorm,
    #[default]
    R8g8b8a8Unorm,
    R8g8b8a8Snorm,
    R16Float,
    R16g16Float,
    R16g16b16a16Float,
    Unknown(String),
}

impl MagpieTextureFormat {
    #[must_use]
    pub fn from_magpiefx(value: Option<&str>) -> Self {
        match value.unwrap_or("R8G8B8A8_UNORM").trim() {
            "R8_UNORM" => Self::R8Unorm,
            "R8G8_UNORM" => Self::R8g8Unorm,
            "R8G8B8A8_UNORM" => Self::R8g8b8a8Unorm,
            "R8G8B8A8_SNORM" => Self::R8g8b8a8Snorm,
            "R16_FLOAT" => Self::R16Float,
            "R16G16_FLOAT" => Self::R16g16Float,
            "R16G16B16A16_FLOAT" => Self::R16g16b16a16Float,
            other => Self::Unknown(other.to_owned()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpiePassPlan {
    pub name: String,
    pub description: Option<String>,
    pub style: MagpieFxPassStyle,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub block_size: Option<Vec<u32>>,
    pub num_threads: Option<Vec<u32>>,
}

#[derive(Clone, Copy)]
struct DimensionContext {
    input_size: FrameSize,
    output_size: FrameSize,
}

fn texture_plan(
    texture: &MagpieFxTexture,
    context: DimensionContext,
) -> UpscaleResult<MagpieTexturePlan> {
    let role = texture_role(texture);
    let width = texture
        .width
        .as_deref()
        .map(|expr| eval_dimension(expr, context))
        .transpose()?;
    let height = texture
        .height
        .as_deref()
        .map(|expr| eval_dimension(expr, context))
        .transpose()?;
    let size = match (width, height, role) {
        (Some(width), Some(height), _) => Some(
            FrameSize::new(width, height)
                .ok_or_else(|| invalid_pipeline("Magpie texture dimensions must be non-zero"))?,
        ),
        (None, None, MagpieTextureRole::Input) => Some(context.input_size),
        (None, None, MagpieTextureRole::Output) => Some(context.output_size),
        (None, None, MagpieTextureRole::SourceAsset | MagpieTextureRole::Intermediate) => None,
        _ => {
            return Err(invalid_pipeline(format!(
                "texture {} must specify both WIDTH and HEIGHT",
                texture.name
            )));
        }
    };

    Ok(MagpieTexturePlan {
        name: texture.name.clone(),
        role,
        size,
        format: MagpieTextureFormat::from_magpiefx(texture.format.as_deref()),
        source: texture.source.clone(),
    })
}

fn texture_role(texture: &MagpieFxTexture) -> MagpieTextureRole {
    if texture.name == "INPUT" {
        MagpieTextureRole::Input
    } else if texture.name == "OUTPUT" {
        MagpieTextureRole::Output
    } else if texture.source.is_some() {
        MagpieTextureRole::SourceAsset
    } else {
        MagpieTextureRole::Intermediate
    }
}

fn validate_texture_refs(
    names: &[String],
    texture_names: &HashSet<&str>,
) -> UpscaleResult<Vec<String>> {
    names
        .iter()
        .map(|name| {
            texture_names
                .contains(name.as_str())
                .then(|| name.clone())
                .ok_or_else(|| invalid_pipeline(format!("pass references unknown texture {name}")))
        })
        .collect()
}

fn eval_dimension(expr: &str, context: DimensionContext) -> UpscaleResult<u32> {
    expr.split('*').map(str::trim).try_fold(1_u32, |acc, part| {
        let value = eval_dimension_factor(part, context)?;
        acc.checked_mul(value)
            .ok_or_else(|| invalid_pipeline(format!("dimension expression overflowed: {expr}")))
    })
}

fn eval_dimension_factor(part: &str, context: DimensionContext) -> UpscaleResult<u32> {
    match part {
        "INPUT_WIDTH" => Ok(context.input_size.width),
        "INPUT_HEIGHT" => Ok(context.input_size.height),
        "OUTPUT_WIDTH" => Ok(context.output_size.width),
        "OUTPUT_HEIGHT" => Ok(context.output_size.height),
        _ => part.parse::<u32>().map_err(|_| {
            invalid_pipeline(format!("unsupported dimension expression token: {part}"))
        }),
    }
}

fn invalid_pipeline(message: impl Into<String>) -> UpscaleError {
    UpscaleError::InvalidPipeline(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_magpiefx;

    #[test]
    fn resolves_input_output_and_scaled_intermediate_textures() {
        let source = r"
//!MAGPIE EFFECT
//!VERSION 4
//!TEXTURE
Texture2D INPUT;
//!TEXTURE
Texture2D OUTPUT;
//!TEXTURE
//!WIDTH INPUT_WIDTH * 2 * 1
//!HEIGHT INPUT_HEIGHT * 2
//!FORMAT R16G16B16A16_FLOAT
Texture2D tex1;
//!PASS 1
//!IN INPUT
//!OUT tex1
//!PASS 2
//!IN tex1
//!OUT OUTPUT
void Pass1() {}
";
        let effect = parse_magpiefx(source).unwrap();
        let plan = MagpieRenderPlan::from_effect(
            &effect,
            FrameSize::new(640, 480).unwrap(),
            FrameSize::new(1280, 960).unwrap(),
        )
        .unwrap();

        assert_eq!(
            plan.texture("INPUT").and_then(|texture| texture.size),
            FrameSize::new(640, 480)
        );
        assert_eq!(
            plan.texture("OUTPUT").and_then(|texture| texture.size),
            FrameSize::new(1280, 960)
        );
        assert_eq!(
            plan.texture("tex1").and_then(|texture| texture.size),
            FrameSize::new(1280, 960)
        );
        assert_eq!(
            plan.texture("tex1").map(|texture| &texture.format),
            Some(&MagpieTextureFormat::R16g16b16a16Float)
        );
    }

    #[test]
    fn keeps_source_asset_texture_size_unresolved() {
        let source = r"
//!MAGPIE EFFECT
//!VERSION 4
//!TEXTURE
Texture2D INPUT;
//!TEXTURE
Texture2D OUTPUT;
//!TEXTURE
//!SOURCE AreaTex.dds
//!FORMAT R8_UNORM
Texture2D areaTex;
//!PASS 1
//!IN INPUT, areaTex
//!OUT OUTPUT
void Pass1() {}
";
        let effect = parse_magpiefx(source).unwrap();
        let plan = MagpieRenderPlan::from_effect(
            &effect,
            FrameSize::new(640, 480).unwrap(),
            FrameSize::new(1280, 960).unwrap(),
        )
        .unwrap();

        let area_tex = plan.texture("areaTex").unwrap();
        assert_eq!(area_tex.role, MagpieTextureRole::SourceAsset);
        assert_eq!(area_tex.size, None);
        assert_eq!(area_tex.source.as_deref(), Some("AreaTex.dds"));
    }

    #[test]
    fn rejects_unknown_pass_texture_reference() {
        let source = r"
//!MAGPIE EFFECT
//!VERSION 4
//!TEXTURE
Texture2D INPUT;
//!TEXTURE
Texture2D OUTPUT;
//!PASS 1
//!IN missing
//!OUT OUTPUT
void Pass1() {}
";
        let effect = parse_magpiefx(source).unwrap();
        let result = MagpieRenderPlan::from_effect(
            &effect,
            FrameSize::new(640, 480).unwrap(),
            FrameSize::new(1280, 960).unwrap(),
        );

        assert!(matches!(result, Err(UpscaleError::InvalidPipeline(_))));
    }
}
