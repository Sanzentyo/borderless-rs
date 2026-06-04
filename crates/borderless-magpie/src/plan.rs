// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: Copyright (c) Xu and Magpie contributors
// SPDX-FileCopyrightText: Copyright (c) Borderless Oxide contributors
// Magpie-compatible render resource planning based on the GPL-licensed Magpie project:
// https://github.com/Blinue/Magpie

use crate::dds::read_dds_metadata;
use crate::magpiefx::{MagpieFx, MagpieFxPassStyle, MagpieFxTexture, parse_magpiefx_file};
use borderless_upscale_core::{FrameSize, UpscaleError, UpscaleResult};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

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
        Self::from_effect_with_source_dir(effect, None, input_size, output_size)
    }

    pub fn from_effect_with_source_dir(
        effect: &MagpieFx,
        source_dir: Option<&Path>,
        input_size: FrameSize,
        output_size: FrameSize,
    ) -> UpscaleResult<Self> {
        effect.validate()?;
        let context = DimensionContext {
            input_size,
            output_size,
            source_dir,
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
        let texture_roles = textures
            .iter()
            .map(|texture| (texture.name.as_str(), texture.role))
            .collect::<HashMap<_, _>>();
        let pass_count = effect.passes.len();
        let passes = effect
            .passes
            .iter()
            .enumerate()
            .map(|(pass_index, pass)| {
                let inputs = validate_texture_refs(&pass.inputs, &texture_names)?;
                let outputs = validate_texture_refs(&pass.outputs, &texture_names)?;
                validate_pass_io(pass_index, pass_count, &inputs, &outputs, &texture_roles)?;
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

    pub fn from_effect_file(
        path: impl AsRef<Path>,
        input_size: FrameSize,
        output_size: FrameSize,
    ) -> UpscaleResult<Self> {
        let path = path.as_ref();
        let effect = parse_magpiefx_file(path)?;
        let source_dir = path.parent();
        Self::from_effect_with_source_dir(&effect, source_dir, input_size, output_size)
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
    pub source_path: Option<PathBuf>,
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
    R8g8b8a8UnormSrgb,
    R8g8b8a8Snorm,
    R16Float,
    R16g16Float,
    R16g16b16a16Float,
    R32Float,
    R32g32b32a32Float,
    Dxgi(u32),
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

    #[must_use]
    pub fn from_magpiefx_or_source(value: Option<&str>, source: Option<Self>) -> Self {
        match (value, source) {
            (None, Some(source)) => source,
            _ => Self::from_magpiefx(value),
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
struct DimensionContext<'a> {
    input_size: FrameSize,
    output_size: FrameSize,
    source_dir: Option<&'a Path>,
}

fn texture_plan(
    texture: &MagpieFxTexture,
    context: DimensionContext<'_>,
) -> UpscaleResult<MagpieTexturePlan> {
    let role = texture_role(texture);
    let source_path = texture
        .source
        .as_deref()
        .and_then(|source| context.source_dir.map(|source_dir| source_dir.join(source)));
    let source_metadata = source_path.as_deref().map(read_dds_metadata).transpose()?;
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
        (None, None, MagpieTextureRole::SourceAsset) => {
            source_metadata.as_ref().map(|metadata| metadata.size)
        }
        (None, None, MagpieTextureRole::Intermediate) => None,
        _ => {
            return Err(invalid_pipeline(format!(
                "texture {} must specify both WIDTH and HEIGHT",
                texture.name
            )));
        }
    };

    let source_format = source_metadata.and_then(|metadata| metadata.format);
    Ok(MagpieTexturePlan {
        name: texture.name.clone(),
        role,
        size,
        format: MagpieTextureFormat::from_magpiefx_or_source(
            texture.format.as_deref(),
            source_format,
        ),
        source: texture.source.clone(),
        source_path,
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

fn validate_pass_io(
    pass_index: usize,
    pass_count: usize,
    inputs: &[String],
    outputs: &[String],
    texture_roles: &HashMap<&str, MagpieTextureRole>,
) -> UpscaleResult<()> {
    if inputs.is_empty() {
        return Err(invalid_pipeline("Magpie pass requires at least one input"));
    }
    if outputs.is_empty() {
        return Err(invalid_pipeline("Magpie pass requires at least one output"));
    }
    validate_unique_refs(inputs, "input")?;
    validate_unique_refs(outputs, "output")?;
    validate_disjoint_refs(inputs, outputs)?;
    for input in inputs {
        if matches!(
            texture_roles.get(input.as_str()),
            Some(MagpieTextureRole::Output)
        ) {
            return Err(invalid_pipeline("Magpie pass cannot read OUTPUT texture"));
        }
    }
    if pass_index + 1 == pass_count {
        return validate_final_pass_outputs(outputs);
    }
    validate_intermediate_pass_outputs(outputs, texture_roles)
}

fn validate_unique_refs(names: &[String], kind: &str) -> UpscaleResult<()> {
    let mut seen = HashSet::new();
    for name in names {
        if !seen.insert(name.as_str()) {
            return Err(invalid_pipeline(format!(
                "Magpie pass has duplicate {kind} texture {name}"
            )));
        }
    }
    Ok(())
}

fn validate_disjoint_refs(inputs: &[String], outputs: &[String]) -> UpscaleResult<()> {
    let inputs = inputs.iter().map(String::as_str).collect::<HashSet<_>>();
    for output in outputs {
        if inputs.contains(output.as_str()) {
            return Err(invalid_pipeline(format!(
                "Magpie pass cannot read and write texture {output}"
            )));
        }
    }
    Ok(())
}

fn validate_final_pass_outputs(outputs: &[String]) -> UpscaleResult<()> {
    if outputs == ["OUTPUT"] {
        Ok(())
    } else {
        Err(invalid_pipeline(
            "final Magpie pass must output exactly OUTPUT",
        ))
    }
}

fn validate_intermediate_pass_outputs(
    outputs: &[String],
    texture_roles: &HashMap<&str, MagpieTextureRole>,
) -> UpscaleResult<()> {
    if outputs.len() > 8 {
        return Err(invalid_pipeline(
            "Magpie pass cannot output more than 8 textures",
        ));
    }
    for output in outputs {
        match texture_roles.get(output.as_str()) {
            Some(MagpieTextureRole::Intermediate) => {}
            Some(MagpieTextureRole::Input | MagpieTextureRole::Output) => {
                return Err(invalid_pipeline(format!(
                    "intermediate Magpie pass cannot output {output}"
                )));
            }
            Some(MagpieTextureRole::SourceAsset) => {
                return Err(invalid_pipeline(format!(
                    "Magpie pass cannot output source asset texture {output}"
                )));
            }
            None => {
                return Err(invalid_pipeline(format!(
                    "pass references unknown texture {output}"
                )));
            }
        }
    }
    Ok(())
}

fn eval_dimension(expr: &str, context: DimensionContext<'_>) -> UpscaleResult<u32> {
    expr.split('*').map(str::trim).try_fold(1_u32, |acc, part| {
        let value = eval_dimension_factor(part, context)?;
        acc.checked_mul(value)
            .ok_or_else(|| invalid_pipeline(format!("dimension expression overflowed: {expr}")))
    })
}

fn eval_dimension_factor(part: &str, context: DimensionContext<'_>) -> UpscaleResult<u32> {
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
    fn resolves_source_asset_metadata_from_effect_directory() {
        let root = std::env::temp_dir().join(format!(
            "borderless-magpie-plan-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("AreaTex.dds"), fake_dds_dx10(160, 560, 61)).unwrap();

        let source = r"
//!MAGPIE EFFECT
//!VERSION 4
//!TEXTURE
Texture2D INPUT;
//!TEXTURE
Texture2D OUTPUT;
//!TEXTURE
//!SOURCE AreaTex.dds
Texture2D areaTex;
//!PASS 1
//!IN INPUT, areaTex
//!OUT OUTPUT
void Pass1() {}
";
        let effect = parse_magpiefx(source).unwrap();
        let plan = MagpieRenderPlan::from_effect_with_source_dir(
            &effect,
            Some(&root),
            FrameSize::new(640, 480).unwrap(),
            FrameSize::new(1280, 960).unwrap(),
        )
        .unwrap();

        let area_tex = plan.texture("areaTex").unwrap();
        assert_eq!(area_tex.size, FrameSize::new(160, 560));
        assert_eq!(area_tex.format, MagpieTextureFormat::R8Unorm);
        assert!(
            area_tex
                .source_path
                .as_ref()
                .is_some_and(|path| path.exists())
        );

        let _ = std::fs::remove_dir_all(root);
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

    #[test]
    fn rejects_output_as_input_texture() {
        let source = r"
//!MAGPIE EFFECT
//!VERSION 4
//!TEXTURE
Texture2D INPUT;
//!TEXTURE
Texture2D OUTPUT;
//!PASS 1
//!IN OUTPUT
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

    #[test]
    fn rejects_intermediate_pass_outputting_source_asset() {
        let source = r"
//!MAGPIE EFFECT
//!VERSION 4
//!TEXTURE
Texture2D INPUT;
//!TEXTURE
Texture2D OUTPUT;
//!TEXTURE
//!SOURCE Lut.dds
Texture2D lut;
//!PASS 1
//!IN INPUT
//!OUT lut
//!PASS 2
//!IN INPUT
//!OUT OUTPUT
void Pass1() {}
void Pass2() {}
";
        let effect = parse_magpiefx(source).unwrap();
        let result = MagpieRenderPlan::from_effect(
            &effect,
            FrameSize::new(640, 480).unwrap(),
            FrameSize::new(1280, 960).unwrap(),
        );

        assert!(matches!(result, Err(UpscaleError::InvalidPipeline(_))));
    }

    #[test]
    fn rejects_non_final_pass_outputting_output_texture() {
        let source = r"
//!MAGPIE EFFECT
//!VERSION 4
//!TEXTURE
Texture2D INPUT;
//!TEXTURE
Texture2D OUTPUT;
//!TEXTURE
Texture2D tmp;
//!PASS 1
//!IN INPUT
//!OUT OUTPUT
//!PASS 2
//!IN tmp
//!OUT OUTPUT
void Pass1() {}
void Pass2() {}
";
        let effect = parse_magpiefx(source).unwrap();
        let result = MagpieRenderPlan::from_effect(
            &effect,
            FrameSize::new(640, 480).unwrap(),
            FrameSize::new(1280, 960).unwrap(),
        );

        assert!(matches!(result, Err(UpscaleError::InvalidPipeline(_))));
    }

    fn fake_dds_dx10(width: u32, height: u32, dxgi_format: u32) -> Vec<u8> {
        const DDS_MAGIC: &[u8; 4] = b"DDS ";
        const MIN_DDS_SIZE: usize = 128;
        const DDPF_FOURCC: u32 = 0x0000_0004;
        const DX10_FOURCC: u32 = u32::from_le_bytes(*b"DX10");

        let mut bytes = vec![0_u8; MIN_DDS_SIZE + 20];
        bytes[..4].copy_from_slice(DDS_MAGIC);
        write_u32(&mut bytes, 4, 124);
        write_u32(&mut bytes, 12, height);
        write_u32(&mut bytes, 16, width);
        write_u32(&mut bytes, 76, 32);
        write_u32(&mut bytes, 80, DDPF_FOURCC);
        write_u32(&mut bytes, 84, DX10_FOURCC);
        write_u32(&mut bytes, MIN_DDS_SIZE, dxgi_format);
        bytes
    }

    fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
}
