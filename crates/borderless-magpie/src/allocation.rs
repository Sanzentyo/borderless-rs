// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: Copyright (c) Xu and Magpie contributors
// SPDX-FileCopyrightText: Copyright (c) Borderless Oxide contributors
// Magpie-compatible texture allocation planning for the GPL port:
// https://github.com/Blinue/Magpie

use crate::formats::MagpieTextureFormatDescriptor;
use crate::plan::{MagpieRenderPlan, MagpieTextureFormat, MagpieTexturePlan, MagpieTextureRole};
use borderless_upscale_core::{FrameSize, UpscaleError, UpscaleResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieTextureAllocationPlan {
    pub textures: Vec<MagpieTextureAllocation>,
}

impl MagpieTextureAllocationPlan {
    pub fn from_render_plan(render_plan: &MagpieRenderPlan) -> UpscaleResult<Self> {
        let lifetimes = texture_lifetimes(render_plan);
        let textures = render_plan
            .textures
            .iter()
            .map(|texture| texture_allocation(texture, &lifetimes))
            .collect::<UpscaleResult<Vec<_>>>()?;
        Ok(Self { textures })
    }

    #[must_use]
    pub fn texture(&self, name: &str) -> Option<&MagpieTextureAllocation> {
        self.textures.iter().find(|texture| texture.name == name)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieTextureAllocation {
    pub name: String,
    pub role: MagpieTextureRole,
    pub format: MagpieTextureFormat,
    pub format_descriptor: MagpieTextureFormatDescriptor,
    pub size: FrameSize,
    pub source_path: Option<PathBuf>,
    pub usage: Vec<MagpieTextureUsage>,
    pub reads: Vec<u32>,
    pub writes: Vec<u32>,
    pub first_use_pass: Option<u32>,
    pub last_use_pass: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MagpieTextureUsage {
    ExternalInput,
    ExternalOutput,
    ShaderResource,
    UnorderedAccess,
    SourceUpload,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct TextureLifetime {
    reads: Vec<u32>,
    writes: Vec<u32>,
}

fn texture_lifetimes(render_plan: &MagpieRenderPlan) -> HashMap<&str, TextureLifetime> {
    let mut lifetimes = render_plan
        .textures
        .iter()
        .map(|texture| (texture.name.as_str(), TextureLifetime::default()))
        .collect::<HashMap<_, _>>();

    for (index, pass) in render_plan.passes.iter().enumerate() {
        let pass_index = u32::try_from(index + 1).unwrap_or(u32::MAX);
        for input in &pass.inputs {
            if let Some(lifetime) = lifetimes.get_mut(input.as_str()) {
                lifetime.reads.push(pass_index);
            }
        }
        for output in &pass.outputs {
            if let Some(lifetime) = lifetimes.get_mut(output.as_str()) {
                lifetime.writes.push(pass_index);
            }
        }
    }

    lifetimes
}

fn texture_allocation(
    texture: &MagpieTexturePlan,
    lifetimes: &HashMap<&str, TextureLifetime>,
) -> UpscaleResult<MagpieTextureAllocation> {
    let lifetime = lifetimes
        .get(texture.name.as_str())
        .cloned()
        .unwrap_or_default();
    let size = texture.size.ok_or_else(|| {
        invalid_pipeline(format!(
            "texture {} has unresolved size and cannot be allocated",
            texture.name
        ))
    })?;
    let usage = texture_usage(texture.role, &lifetime);
    let first_use_pass = first_use(&lifetime);
    let last_use_pass = last_use(&lifetime);

    Ok(MagpieTextureAllocation {
        name: texture.name.clone(),
        role: texture.role,
        format: texture.format.clone(),
        format_descriptor: texture.format.descriptor(),
        size,
        source_path: texture.source_path.clone(),
        usage,
        reads: lifetime.reads,
        writes: lifetime.writes,
        first_use_pass,
        last_use_pass,
    })
}

fn texture_usage(role: MagpieTextureRole, lifetime: &TextureLifetime) -> Vec<MagpieTextureUsage> {
    let mut usage = Vec::new();
    if role == MagpieTextureRole::Input {
        usage.push(MagpieTextureUsage::ExternalInput);
    }
    if role == MagpieTextureRole::Output {
        usage.push(MagpieTextureUsage::ExternalOutput);
    }
    if !lifetime.reads.is_empty() {
        usage.push(MagpieTextureUsage::ShaderResource);
    }
    if !lifetime.writes.is_empty() {
        usage.push(MagpieTextureUsage::UnorderedAccess);
    }
    if role == MagpieTextureRole::SourceAsset {
        usage.push(MagpieTextureUsage::SourceUpload);
    }
    usage
}

fn first_use(lifetime: &TextureLifetime) -> Option<u32> {
    lifetime
        .reads
        .iter()
        .chain(lifetime.writes.iter())
        .copied()
        .min()
}

fn last_use(lifetime: &TextureLifetime) -> Option<u32> {
    lifetime
        .reads
        .iter()
        .chain(lifetime.writes.iter())
        .copied()
        .max()
}

fn invalid_pipeline(message: impl Into<String>) -> UpscaleError {
    UpscaleError::InvalidPipeline(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::magpiefx::parse_magpiefx;

    #[test]
    fn plans_texture_usage_and_lifetimes() {
        let render_plan = render_plan(
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
//!IN INPUT
//!OUT tex1
//!STYLE PS
MF4 Pass1(float2 pos) { return INPUT.SampleLevel(SamplerState{}, pos, 0); }
//!PASS 2
//!IN tex1
//!OUT OUTPUT
//!BLOCK_SIZE 8
//!NUM_THREADS 64
void Pass2(uint2 pos) { OUTPUT[pos] = tex1[pos]; }
",
        );

        let plan = MagpieTextureAllocationPlan::from_render_plan(&render_plan).unwrap();

        let input = plan.texture("INPUT").unwrap();
        assert!(input.usage.contains(&MagpieTextureUsage::ExternalInput));
        assert!(input.usage.contains(&MagpieTextureUsage::ShaderResource));
        assert_eq!(input.reads, vec![1]);
        assert_eq!(input.writes, Vec::<u32>::new());
        assert_eq!(input.first_use_pass, Some(1));
        assert_eq!(input.last_use_pass, Some(1));

        let intermediate = plan.texture("tex1").unwrap();
        assert!(
            intermediate
                .usage
                .contains(&MagpieTextureUsage::ShaderResource)
        );
        assert!(
            intermediate
                .usage
                .contains(&MagpieTextureUsage::UnorderedAccess)
        );
        assert_eq!(intermediate.writes, vec![1]);
        assert_eq!(intermediate.reads, vec![2]);
        assert_eq!(intermediate.format_descriptor.dxgi_format, Some(28));
        assert_eq!(intermediate.format_descriptor.bytes_per_pixel, Some(4));
        assert_eq!(intermediate.first_use_pass, Some(1));
        assert_eq!(intermediate.last_use_pass, Some(2));

        let output = plan.texture("OUTPUT").unwrap();
        assert!(output.usage.contains(&MagpieTextureUsage::ExternalOutput));
        assert!(output.usage.contains(&MagpieTextureUsage::UnorderedAccess));
        assert_eq!(output.writes, vec![2]);
    }

    #[test]
    fn rejects_unresolved_intermediate_texture_size() {
        let render_plan = render_plan(
            r"
//!MAGPIE EFFECT
//!VERSION 4
//!TEXTURE
Texture2D INPUT;
//!TEXTURE
Texture2D tex1;
//!TEXTURE
Texture2D OUTPUT;
//!PASS 1
//!IN INPUT
//!OUT tex1
//!STYLE PS
MF4 Pass1(float2 pos) { return INPUT.SampleLevel(SamplerState{}, pos, 0); }
//!PASS 2
//!IN tex1
//!OUT OUTPUT
//!BLOCK_SIZE 8
//!NUM_THREADS 64
void Pass2(uint2 pos) { OUTPUT[pos] = tex1[pos]; }
",
        );

        assert!(matches!(
            MagpieTextureAllocationPlan::from_render_plan(&render_plan),
            Err(UpscaleError::InvalidPipeline(_))
        ));
    }

    fn render_plan(source: &str) -> MagpieRenderPlan {
        let effect = parse_magpiefx(source).unwrap();
        let input_size = FrameSize::new(320, 240).unwrap();
        let output_size = FrameSize::new(640, 480).unwrap();
        MagpieRenderPlan::from_effect(&effect, input_size, output_size).unwrap()
    }
}
