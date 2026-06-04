// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: Copyright (c) Xu and Magpie contributors
// SPDX-FileCopyrightText: Copyright (c) Borderless Oxide contributors
// Magpie-compatible compute dispatch planning for the GPL port:
// https://github.com/Blinue/Magpie

use crate::magpiefx::MagpieFxPassStyle;
use crate::plan::{MagpiePassPlan, MagpieRenderPlan, MagpieTexturePlan};
use borderless_upscale_core::{FrameSize, UpscaleError, UpscaleResult};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieDispatchPlan {
    pub passes: Vec<MagpieDispatchPass>,
}

impl MagpieDispatchPlan {
    pub fn from_render_plan(render_plan: &MagpieRenderPlan) -> UpscaleResult<Self> {
        let passes = render_plan
            .passes
            .iter()
            .enumerate()
            .map(|(index, pass)| dispatch_pass(index + 1, pass, &render_plan.textures))
            .collect::<UpscaleResult<Vec<_>>>()?;
        Ok(Self { passes })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieDispatchPass {
    pub pass_index: u32,
    pub pass_name: String,
    pub output_texture: String,
    pub output_size: FrameSize,
    pub block_size: [u32; 2],
    pub group_count: [u32; 3],
}

fn dispatch_pass(
    pass_index: usize,
    pass: &MagpiePassPlan,
    textures: &[MagpieTexturePlan],
) -> UpscaleResult<MagpieDispatchPass> {
    let pass_index = u32::try_from(pass_index).map_err(|_| invalid_pipeline("too many passes"))?;
    let output_texture = pass
        .outputs
        .first()
        .ok_or_else(|| invalid_pipeline(format!("pass {pass_index} has no output texture")))?;
    let output = textures
        .iter()
        .find(|texture| texture.name == *output_texture)
        .ok_or_else(|| {
            invalid_pipeline(format!(
                "pass {pass_index} references unknown output texture {output_texture}"
            ))
        })?;
    let output_size = output.size.ok_or_else(|| {
        invalid_pipeline(format!(
            "pass {pass_index} output texture {output_texture} has unresolved size"
        ))
    })?;
    let block_size = normalized_block_size(pass)?;
    Ok(MagpieDispatchPass {
        pass_index,
        pass_name: pass
            .description
            .clone()
            .unwrap_or_else(|| pass.name.clone()),
        output_texture: output_texture.clone(),
        output_size,
        block_size,
        group_count: [
            div_ceil(output_size.width, block_size[0]),
            div_ceil(output_size.height, block_size[1]),
            1,
        ],
    })
}

fn normalized_block_size(pass: &MagpiePassPlan) -> UpscaleResult<[u32; 2]> {
    if pass.style == MagpieFxPassStyle::PixelShader {
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

const fn div_ceil(value: u32, divisor: u32) -> u32 {
    value.div_ceil(divisor)
}

fn invalid_pipeline(message: impl Into<String>) -> UpscaleError {
    UpscaleError::InvalidPipeline(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::magpiefx::parse_magpiefx;
    use borderless_upscale_core::FrameSize;

    #[test]
    fn dispatches_pixel_style_pass_by_output_size_over_sixteen() {
        let render_plan = render_plan(
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
            FrameSize::new(640, 480).unwrap(),
            FrameSize::new(1281, 721).unwrap(),
        );

        let dispatch = MagpieDispatchPlan::from_render_plan(&render_plan).unwrap();

        assert_eq!(dispatch.passes[0].block_size, [16, 16]);
        assert_eq!(dispatch.passes[0].group_count, [81, 46, 1]);
    }

    #[test]
    fn dispatches_compute_pass_by_first_output_size_over_block_size() {
        let render_plan = render_plan(
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
//!BLOCK_SIZE 16, 8
//!NUM_THREADS 64
void Pass1(uint2 blockStart, uint3 threadId) {}
",
            FrameSize::new(640, 480).unwrap(),
            FrameSize::new(1280, 721).unwrap(),
        );

        let dispatch = MagpieDispatchPlan::from_render_plan(&render_plan).unwrap();

        assert_eq!(dispatch.passes[0].block_size, [16, 8]);
        assert_eq!(dispatch.passes[0].group_count, [80, 91, 1]);
    }

    fn render_plan(
        source: &str,
        input_size: FrameSize,
        output_size: FrameSize,
    ) -> MagpieRenderPlan {
        let effect = parse_magpiefx(source).unwrap();
        MagpieRenderPlan::from_effect(&effect, input_size, output_size).unwrap()
    }
}
