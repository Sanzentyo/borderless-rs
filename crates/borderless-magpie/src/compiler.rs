// SPDX-License-Identifier: GPL-3.0-or-later
// Magpie-compatible shader compilation planning for the GPL port:
// https://github.com/Blinue/Magpie

use crate::magpiefx::MagpieFxPassStyle;
use crate::package::MagpieEffectPackage;
use crate::plan::MagpiePassPlan;
use borderless_upscale_core::{UpscaleError, UpscaleResult};
use serde::{Deserialize, Serialize};

pub const MAGPIE_ENTRY_POINT: &str = "__M";
pub const MAGPIE_TARGET_PROFILE: &str = "cs_5_0";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieCompilePlan {
    pub source_name: String,
    pub jobs: Vec<MagpieShaderJob>,
}

impl MagpieCompilePlan {
    pub fn from_package(package: &MagpieEffectPackage) -> UpscaleResult<Self> {
        let source_name = package
            .effect_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("effect.hlsl")
            .to_owned();
        let jobs = package
            .render_plan
            .passes
            .iter()
            .enumerate()
            .map(|(index, pass)| {
                shader_job(
                    index + 1,
                    pass,
                    &package.compiler_source,
                    &package.effect.capabilities,
                )
            })
            .collect::<UpscaleResult<Vec<_>>>()?;

        Ok(Self { source_name, jobs })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieShaderJob {
    pub pass_index: u32,
    pub pass_name: String,
    pub pass_function: String,
    pub entry_point: String,
    pub target_profile: String,
    pub style: MagpieFxPassStyle,
    pub block_size: [u32; 2],
    pub num_threads: [u32; 3],
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub base_source: String,
    pub macros: Vec<MagpieShaderMacro>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieShaderMacro {
    pub name: String,
    pub value: Option<String>,
}

impl MagpieShaderMacro {
    #[must_use]
    pub fn define(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: None,
        }
    }

    #[must_use]
    pub fn value(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: Some(value.into()),
        }
    }
}

fn shader_job(
    pass_index: usize,
    pass: &MagpiePassPlan,
    base_source: &str,
    capabilities: &[String],
) -> UpscaleResult<MagpieShaderJob> {
    let block_size = normalized_block_size(pass)?;
    let num_threads = normalized_num_threads(pass)?;
    let pass_index = u32::try_from(pass_index).map_err(|_| invalid_pipeline("too many passes"))?;
    let mut macros = vec![
        MagpieShaderMacro::value("MP_BLOCK_WIDTH", block_size[0].to_string()),
        MagpieShaderMacro::value("MP_BLOCK_HEIGHT", block_size[1].to_string()),
        MagpieShaderMacro::value("MP_NUM_THREADS_X", num_threads[0].to_string()),
        MagpieShaderMacro::value("MP_NUM_THREADS_Y", num_threads[1].to_string()),
        MagpieShaderMacro::value("MP_NUM_THREADS_Z", num_threads[2].to_string()),
    ];
    if pass.style == MagpieFxPassStyle::PixelShader {
        macros.push(MagpieShaderMacro::define("MP_PS_STYLE"));
    }
    extend_float_macros(
        &mut macros,
        capabilities.iter().any(|value| value == "FP16"),
    );

    Ok(MagpieShaderJob {
        pass_index,
        pass_name: pass
            .description
            .clone()
            .unwrap_or_else(|| format!("Pass {pass_index}")),
        pass_function: format!("Pass{pass_index}"),
        entry_point: MAGPIE_ENTRY_POINT.to_owned(),
        target_profile: MAGPIE_TARGET_PROFILE.to_owned(),
        style: pass.style,
        block_size,
        num_threads,
        inputs: pass.inputs.clone(),
        outputs: pass.outputs.clone(),
        base_source: base_source.to_owned(),
        macros,
    })
}

fn normalized_block_size(pass: &MagpiePassPlan) -> UpscaleResult<[u32; 2]> {
    if pass.style == MagpieFxPassStyle::PixelShader {
        if pass.block_size.is_some() {
            return Err(invalid_pipeline(
                "PS-style Magpie pass must not specify BLOCK_SIZE",
            ));
        }
        return Ok([16, 16]);
    }
    let block_size = pass
        .block_size
        .as_deref()
        .ok_or_else(|| invalid_pipeline("CS-style Magpie pass requires BLOCK_SIZE"))?;
    match block_size {
        [value] if *value > 0 => Ok([*value, *value]),
        [width, height] if *width > 0 && *height > 0 => Ok([*width, *height]),
        _ => Err(invalid_pipeline("invalid Magpie BLOCK_SIZE")),
    }
}

fn normalized_num_threads(pass: &MagpiePassPlan) -> UpscaleResult<[u32; 3]> {
    if pass.style == MagpieFxPassStyle::PixelShader {
        if pass.num_threads.is_some() {
            return Err(invalid_pipeline(
                "PS-style Magpie pass must not specify NUM_THREADS",
            ));
        }
        return Ok([64, 1, 1]);
    }
    let num_threads = pass
        .num_threads
        .as_deref()
        .ok_or_else(|| invalid_pipeline("CS-style Magpie pass requires NUM_THREADS"))?;
    match num_threads {
        [x] if *x > 0 => Ok([*x, 1, 1]),
        [x, y] if *x > 0 && *y > 0 => Ok([*x, *y, 1]),
        [x, y, z] if *x > 0 && *y > 0 && *z > 0 => Ok([*x, *y, *z]),
        _ => Err(invalid_pipeline("invalid Magpie NUM_THREADS")),
    }
}

fn extend_float_macros(macros: &mut Vec<MagpieShaderMacro>, fp16: bool) {
    if fp16 {
        macros.push(MagpieShaderMacro::define("MP_FP16"));
        macros.push(MagpieShaderMacro::value("MF", "min16float"));
        extend_numeric_float_macros(macros, "min16float");
    } else {
        macros.push(MagpieShaderMacro::value("MF", "float"));
        extend_numeric_float_macros(macros, "float");
    }
}

fn extend_numeric_float_macros(macros: &mut Vec<MagpieShaderMacro>, scalar: &str) {
    (1..=4).for_each(|columns| {
        macros.push(MagpieShaderMacro::value(
            format!("MF{columns}"),
            format!("{scalar}{columns}"),
        ));
        (1..=4).for_each(|rows| {
            macros.push(MagpieShaderMacro::value(
                format!("MF{columns}x{rows}"),
                format!("{scalar}{columns}x{rows}"),
            ));
        });
    });
}

fn invalid_pipeline(message: impl Into<String>) -> UpscaleError {
    UpscaleError::InvalidPipeline(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::MagpieEffectPackage;
    use crate::parse_magpiefx;
    use crate::plan::MagpieRenderPlan;
    use borderless_upscale_core::FrameSize;
    use std::path::PathBuf;

    #[test]
    fn plans_pixel_style_pass_as_magpie_compute_entry() {
        let package = package_from_source(
            r"
//!MAGPIE EFFECT
//!VERSION 4
//!TEXTURE
Texture2D INPUT;
//!TEXTURE
Texture2D OUTPUT;
//!PASS 1
//!STYLE PS
//!IN INPUT
//!OUT OUTPUT
float4 Pass1(float2 pos) { return 1; }
",
        );

        let plan = MagpieCompilePlan::from_package(&package).unwrap();

        assert_eq!(plan.jobs.len(), 1);
        assert_eq!(plan.jobs[0].entry_point, "__M");
        assert_eq!(plan.jobs[0].target_profile, "cs_5_0");
        assert_eq!(plan.jobs[0].block_size, [16, 16]);
        assert_eq!(plan.jobs[0].num_threads, [64, 1, 1]);
        assert!(
            plan.jobs[0]
                .macros
                .iter()
                .any(|shader_macro| shader_macro.name == "MP_PS_STYLE")
        );
    }

    #[test]
    fn plans_compute_pass_threads_and_fp16_macros() {
        let package = package_from_source(
            r"
//!MAGPIE EFFECT
//!VERSION 4
//!CAPABILITY FP16
//!TEXTURE
Texture2D INPUT;
//!TEXTURE
Texture2D OUTPUT;
//!PASS 1
//!DESC Setup
//!IN INPUT
//!OUT OUTPUT
//!BLOCK_SIZE 16, 8
//!NUM_THREADS 64
void Pass1(uint2 blockStart, uint3 threadId) {}
",
        );

        let plan = MagpieCompilePlan::from_package(&package).unwrap();
        let job = &plan.jobs[0];

        assert_eq!(job.pass_name, "Setup");
        assert_eq!(job.block_size, [16, 8]);
        assert_eq!(job.num_threads, [64, 1, 1]);
        assert!(job.macros.iter().any(|shader_macro| {
            shader_macro.name == "MF" && shader_macro.value.as_deref() == Some("min16float")
        }));
    }

    fn package_from_source(source: &str) -> MagpieEffectPackage {
        let effect = parse_magpiefx(source).unwrap();
        let input_size = FrameSize::new(640, 480).unwrap();
        let output_size = FrameSize::new(1280, 960).unwrap();
        let render_plan = MagpieRenderPlan::from_effect(&effect, input_size, output_size).unwrap();
        MagpieEffectPackage {
            effect_path: PathBuf::from("effect.hlsl"),
            effect,
            render_plan,
            compiler_source: source.to_owned(),
            includes: Vec::new(),
            source_assets: Vec::new(),
        }
    }
}
