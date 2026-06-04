// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: Copyright (c) Xu and Magpie contributors
// SPDX-FileCopyrightText: Copyright (c) Borderless Oxide contributors
// Magpie-compatible effect parsing based on the GPL-licensed Magpie project:
// https://github.com/Blinue/Magpie

use crate::effect::{EffectGraph, EffectPass, EffectPassStyle, EffectSource, MagpieEffect};
use borderless_upscale_core::{UpscaleError, UpscaleResult};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieFx {
    pub version: u32,
    pub sort_name: Option<String>,
    pub uses: Vec<String>,
    pub capabilities: Vec<String>,
    pub parameters: Vec<MagpieFxParameter>,
    pub textures: Vec<MagpieFxTexture>,
    pub samplers: Vec<MagpieFxSampler>,
    pub passes: Vec<MagpieFxPass>,
    pub hlsl_source: String,
    pub prelude_source: String,
    pub common_source: String,
}

impl MagpieFx {
    pub fn validate(&self) -> UpscaleResult<()> {
        if self.version == 0 {
            return Err(UpscaleError::InvalidPipeline(
                "MagpieFX version is missing".to_owned(),
            ));
        }
        if self.textures.is_empty() {
            return Err(UpscaleError::InvalidPipeline(
                "MagpieFX effect must declare textures".to_owned(),
            ));
        }
        if self.passes.is_empty() {
            return Err(UpscaleError::InvalidPipeline(
                "MagpieFX effect must declare passes".to_owned(),
            ));
        }
        if !self.textures.iter().any(|texture| texture.name == "INPUT") {
            return Err(UpscaleError::InvalidPipeline(
                "MagpieFX effect must declare INPUT texture".to_owned(),
            ));
        }
        if !self.textures.iter().any(|texture| texture.name == "OUTPUT") {
            return Err(UpscaleError::InvalidPipeline(
                "MagpieFX effect must declare OUTPUT texture".to_owned(),
            ));
        }
        Ok(())
    }

    pub fn to_effect(
        &self,
        name: impl Into<String>,
        source: EffectSource,
    ) -> UpscaleResult<MagpieEffect> {
        self.validate()?;
        let effect = MagpieEffect {
            name: name.into(),
            source,
            passes: self
                .passes
                .iter()
                .map(MagpieFxPass::to_effect_pass)
                .collect(),
        };
        effect.validate()?;
        Ok(effect)
    }

    pub fn to_effect_graph(
        &self,
        name: impl Into<String>,
        source: EffectSource,
    ) -> UpscaleResult<EffectGraph> {
        let graph = EffectGraph::new().with_effect(self.to_effect(name, source)?);
        graph.validate()?;
        Ok(graph)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieFxParameter {
    pub symbol: String,
    pub label: Option<String>,
    pub default_value: Option<String>,
    pub min: Option<String>,
    pub max: Option<String>,
    pub step: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieFxTexture {
    pub name: String,
    pub width: Option<String>,
    pub height: Option<String>,
    pub format: Option<String>,
    pub source: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieFxSampler {
    pub name: String,
    pub filter: MagpieFxSamplerFilter,
    pub address: MagpieFxSamplerAddress,
}

impl Default for MagpieFxSampler {
    fn default() -> Self {
        Self {
            name: String::new(),
            filter: MagpieFxSamplerFilter::Linear,
            address: MagpieFxSamplerAddress::Clamp,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MagpieFxSamplerFilter {
    Point,
    #[default]
    Linear,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MagpieFxSamplerAddress {
    #[default]
    Clamp,
    Wrap,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieFxPass {
    pub index: u32,
    pub description: Option<String>,
    pub style: MagpieFxPassStyle,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub block_size: Option<Vec<u32>>,
    pub num_threads: Option<Vec<u32>>,
    pub source: String,
}

impl Default for MagpieFxPass {
    fn default() -> Self {
        Self {
            index: 0,
            description: None,
            style: MagpieFxPassStyle::Compute,
            inputs: Vec::new(),
            outputs: Vec::new(),
            block_size: None,
            num_threads: None,
            source: String::new(),
        }
    }
}

impl MagpieFxPass {
    #[must_use]
    pub fn to_effect_pass(&self) -> EffectPass {
        EffectPass::new(format!("Pass{}", self.index), 1, 1).with_shader_metadata(
            self.description.clone(),
            self.style.into(),
            self.inputs.clone(),
            self.outputs.clone(),
            self.block_size.clone(),
            self.num_threads.clone(),
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MagpieFxPassStyle {
    PixelShader,
    #[default]
    Compute,
}

impl From<MagpieFxPassStyle> for EffectPassStyle {
    fn from(value: MagpieFxPassStyle) -> Self {
        match value {
            MagpieFxPassStyle::PixelShader => Self::PixelShader,
            MagpieFxPassStyle::Compute => Self::Compute,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Block {
    Common,
    Parameter,
    Texture,
    Sampler,
    Pass,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DirectiveOutcome {
    Ignored,
    Applied(Option<Block>),
}

pub fn parse_magpiefx(source: &str) -> UpscaleResult<MagpieFx> {
    let mut effect = MagpieFx::default();
    let mut saw_header = false;
    let mut current = None;

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(directive) = trimmed.strip_prefix("//!") {
            current = apply_directive(directive.trim(), current, &mut effect)?;
        } else if let Some(block) = current {
            apply_declaration(trimmed, block, &mut effect);
            append_block_source(line, block, &mut effect);
            effect.hlsl_source.push_str(line);
            effect.hlsl_source.push('\n');
        } else if !trimmed.starts_with("//") {
            effect.hlsl_source.push_str(line);
            effect.hlsl_source.push('\n');
            effect.prelude_source.push_str(line);
            effect.prelude_source.push('\n');
        }

        if trimmed == "//!MAGPIE EFFECT" {
            saw_header = true;
        }
    }

    if !saw_header {
        return Err(UpscaleError::InvalidPipeline(
            "MagpieFX header is missing".to_owned(),
        ));
    }
    effect.validate()?;
    Ok(effect)
}

pub fn parse_magpiefx_file(path: impl AsRef<Path>) -> UpscaleResult<MagpieFx> {
    let path = path.as_ref();
    let source = std::fs::read_to_string(path).map_err(|err| {
        UpscaleError::BackendUnavailable(format!(
            "failed to read MagpieFX file {}: {err}",
            path.display()
        ))
    })?;
    parse_magpiefx(&source)
}

fn apply_directive(
    directive: &str,
    current: Option<Block>,
    effect: &mut MagpieFx,
) -> UpscaleResult<Option<Block>> {
    let mut parts = directive.splitn(2, char::is_whitespace);
    let key = parts.next().unwrap_or_default();
    let value = parts.next().unwrap_or_default().trim();

    if let DirectiveOutcome::Applied(next) = apply_global_directive(key, value, current, effect)? {
        return Ok(next);
    }
    if let DirectiveOutcome::Applied(next) = apply_parameter_directive(key, value, current, effect)
    {
        return Ok(next);
    }
    if let DirectiveOutcome::Applied(next) = apply_texture_directive(key, value, current, effect) {
        return Ok(next);
    }
    if let DirectiveOutcome::Applied(next) = apply_sampler_directive(key, value, current, effect) {
        return Ok(next);
    }
    apply_pass_directive(key, value, current, effect)
}

fn apply_global_directive(
    key: &str,
    value: &str,
    current: Option<Block>,
    effect: &mut MagpieFx,
) -> UpscaleResult<DirectiveOutcome> {
    match key {
        "MAGPIE" if value == "EFFECT" => Ok(DirectiveOutcome::Applied(current)),
        "VERSION" => {
            effect.version = value.parse::<u32>().map_err(|_| {
                UpscaleError::InvalidPipeline("invalid MagpieFX VERSION".to_owned())
            })?;
            Ok(DirectiveOutcome::Applied(current))
        }
        "USE" => {
            extend_csv(&mut effect.uses, value);
            Ok(DirectiveOutcome::Applied(current))
        }
        "CAPABILITY" => {
            extend_csv(&mut effect.capabilities, value);
            Ok(DirectiveOutcome::Applied(current))
        }
        "SORT_NAME" => {
            effect.sort_name = Some(value.to_owned());
            Ok(DirectiveOutcome::Applied(current))
        }
        "COMMON" => Ok(DirectiveOutcome::Applied(Some(Block::Common))),
        _ => Ok(DirectiveOutcome::Ignored),
    }
}

fn apply_parameter_directive(
    key: &str,
    value: &str,
    current: Option<Block>,
    effect: &mut MagpieFx,
) -> DirectiveOutcome {
    match key {
        "PARAMETER" => {
            effect.parameters.push(MagpieFxParameter::default());
            DirectiveOutcome::Applied(Some(Block::Parameter))
        }
        "LABEL" => {
            set_last_parameter(effect, |parameter| parameter.label = Some(value.to_owned()));
            DirectiveOutcome::Applied(current)
        }
        "DEFAULT" => {
            set_last_parameter(effect, |parameter| {
                parameter.default_value = Some(value.to_owned());
            });
            DirectiveOutcome::Applied(current)
        }
        "MIN" => {
            set_last_parameter(effect, |parameter| parameter.min = Some(value.to_owned()));
            DirectiveOutcome::Applied(current)
        }
        "MAX" => {
            set_last_parameter(effect, |parameter| parameter.max = Some(value.to_owned()));
            DirectiveOutcome::Applied(current)
        }
        "STEP" => {
            set_last_parameter(effect, |parameter| parameter.step = Some(value.to_owned()));
            DirectiveOutcome::Applied(current)
        }
        _ => DirectiveOutcome::Ignored,
    }
}

fn set_last_parameter(effect: &mut MagpieFx, apply: impl FnOnce(&mut MagpieFxParameter)) {
    if let Some(parameter) = effect.parameters.last_mut() {
        apply(parameter);
    }
}

fn apply_texture_directive(
    key: &str,
    value: &str,
    current: Option<Block>,
    effect: &mut MagpieFx,
) -> DirectiveOutcome {
    match key {
        "TEXTURE" => {
            effect.textures.push(MagpieFxTexture::default());
            DirectiveOutcome::Applied(Some(Block::Texture))
        }
        "WIDTH" => {
            set_last_texture(effect, |texture| texture.width = Some(value.to_owned()));
            DirectiveOutcome::Applied(current)
        }
        "HEIGHT" => {
            set_last_texture(effect, |texture| texture.height = Some(value.to_owned()));
            DirectiveOutcome::Applied(current)
        }
        "FORMAT" => {
            set_last_texture(effect, |texture| texture.format = Some(value.to_owned()));
            DirectiveOutcome::Applied(current)
        }
        "SOURCE" => {
            set_last_texture(effect, |texture| texture.source = Some(value.to_owned()));
            DirectiveOutcome::Applied(current)
        }
        _ => DirectiveOutcome::Ignored,
    }
}

fn set_last_texture(effect: &mut MagpieFx, apply: impl FnOnce(&mut MagpieFxTexture)) {
    if let Some(texture) = effect.textures.last_mut() {
        apply(texture);
    }
}

fn apply_sampler_directive(
    key: &str,
    value: &str,
    current: Option<Block>,
    effect: &mut MagpieFx,
) -> DirectiveOutcome {
    match key {
        "SAMPLER" => {
            effect.samplers.push(MagpieFxSampler::default());
            DirectiveOutcome::Applied(Some(Block::Sampler))
        }
        "FILTER" => {
            set_last_sampler(effect, |sampler| {
                sampler.filter = match value {
                    "POINT" => MagpieFxSamplerFilter::Point,
                    "LINEAR" => MagpieFxSamplerFilter::Linear,
                    _ => sampler.filter,
                };
            });
            DirectiveOutcome::Applied(current)
        }
        "ADDRESS" => {
            set_last_sampler(effect, |sampler| {
                sampler.address = match value {
                    "WRAP" => MagpieFxSamplerAddress::Wrap,
                    "CLAMP" => MagpieFxSamplerAddress::Clamp,
                    _ => sampler.address,
                };
            });
            DirectiveOutcome::Applied(current)
        }
        _ => DirectiveOutcome::Ignored,
    }
}

fn set_last_sampler(effect: &mut MagpieFx, apply: impl FnOnce(&mut MagpieFxSampler)) {
    if let Some(sampler) = effect.samplers.last_mut() {
        apply(sampler);
    }
}

fn apply_pass_directive(
    key: &str,
    value: &str,
    current: Option<Block>,
    effect: &mut MagpieFx,
) -> UpscaleResult<Option<Block>> {
    match key {
        "PASS" => {
            let index = value
                .parse::<u32>()
                .map_err(|_| UpscaleError::InvalidPipeline("invalid MagpieFX PASS".to_owned()))?;
            effect.passes.push(MagpieFxPass {
                index,
                ..MagpieFxPass::default()
            });
            Ok(Some(Block::Pass))
        }
        "STYLE" => {
            if let Some(pass) = effect.passes.last_mut() {
                pass.style = if value == "PS" {
                    MagpieFxPassStyle::PixelShader
                } else {
                    MagpieFxPassStyle::Compute
                };
            }
            Ok(current)
        }
        "DESC" => {
            if let Some(pass) = effect.passes.last_mut() {
                pass.description = Some(value.to_owned());
            }
            Ok(current)
        }
        "IN" => {
            if let Some(pass) = effect.passes.last_mut() {
                pass.inputs = split_csv(value);
            }
            Ok(current)
        }
        "OUT" => {
            if let Some(pass) = effect.passes.last_mut() {
                pass.outputs = split_csv(value);
            }
            Ok(current)
        }
        "BLOCK_SIZE" => {
            if let Some(pass) = effect.passes.last_mut() {
                pass.block_size = Some(parse_u32_list(value)?);
            }
            Ok(current)
        }
        "NUM_THREADS" => {
            if let Some(pass) = effect.passes.last_mut() {
                pass.num_threads = Some(parse_u32_list(value)?);
            }
            Ok(current)
        }
        _ => Ok(current),
    }
}

fn apply_declaration(line: &str, block: Block, effect: &mut MagpieFx) {
    match block {
        Block::Common | Block::Pass => {}
        Block::Parameter => {
            if let Some(parameter) = effect.parameters.last_mut()
                && parameter.symbol.is_empty()
                && let Some(symbol) = declaration_symbol(line)
            {
                parameter.symbol = symbol;
            }
        }
        Block::Texture => {
            if let Some(texture) = effect.textures.last_mut()
                && texture.name.is_empty()
                && let Some(symbol) = declaration_symbol(line)
            {
                texture.name = symbol;
            }
        }
        Block::Sampler => {
            if let Some(sampler) = effect.samplers.last_mut()
                && sampler.name.is_empty()
                && let Some(symbol) = declaration_symbol(line)
            {
                sampler.name = symbol;
            }
        }
    }
}

fn append_block_source(line: &str, block: Block, effect: &mut MagpieFx) {
    match block {
        Block::Common => {
            effect.common_source.push_str(line);
            effect.common_source.push('\n');
        }
        Block::Pass => {
            if let Some(pass) = effect.passes.last_mut() {
                pass.source.push_str(line);
                pass.source.push('\n');
            }
        }
        Block::Parameter | Block::Texture | Block::Sampler => {}
    }
}

fn declaration_symbol(line: &str) -> Option<String> {
    let declaration = line.split("//").next()?.trim().trim_end_matches(';').trim();
    declaration
        .split(|ch: char| ch.is_whitespace() || ch == '=')
        .filter(|part| !part.is_empty())
        .nth(1)
        .map(ToOwned::to_owned)
}

fn extend_csv(target: &mut Vec<String>, value: &str) {
    target.extend(split_csv(value));
}

fn split_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn parse_u32_list(value: &str) -> UpscaleResult<Vec<u32>> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            value.parse::<u32>().map_err(|_| {
                UpscaleError::InvalidPipeline(format!("invalid MagpieFX integer: {value}"))
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const NEAREST: &str = r"
//!MAGPIE EFFECT
//!VERSION 4

//!TEXTURE
Texture2D INPUT;

//!TEXTURE
Texture2D OUTPUT;

//!SAMPLER
//!FILTER POINT
SamplerState sam;

//!PASS 1
//!STYLE PS
//!IN INPUT
//!OUT OUTPUT
float4 Pass1(float2 pos) {
    return INPUT.SampleLevel(sam, pos, 0);
}
";

    #[test]
    fn parses_nearest_like_effect() {
        let effect = parse_magpiefx(NEAREST).unwrap();

        assert_eq!(effect.version, 4);
        assert_eq!(effect.textures.len(), 2);
        assert_eq!(effect.samplers[0].filter, MagpieFxSamplerFilter::Point);
        assert_eq!(effect.passes[0].style, MagpieFxPassStyle::PixelShader);
        assert_eq!(effect.passes[0].inputs, ["INPUT"]);
        assert_eq!(effect.passes[0].outputs, ["OUTPUT"]);
        assert!(effect.hlsl_source.contains("Texture2D INPUT;"));
        assert!(!effect.hlsl_source.contains("//!PASS"));
    }

    #[test]
    fn converts_magpiefx_to_effect_graph() {
        let effect = parse_magpiefx(NEAREST).unwrap();
        let graph = effect
            .to_effect_graph(
                "nearest",
                EffectSource::HlslSource {
                    source: effect.hlsl_source.clone(),
                },
            )
            .unwrap();

        let nearest = &graph.effects()[0];
        assert_eq!(nearest.name, "nearest");
        assert_eq!(nearest.passes[0].name, "Pass1");
        assert_eq!(nearest.passes[0].style, EffectPassStyle::PixelShader);
        assert_eq!(nearest.passes[0].inputs, ["INPUT"]);
        assert_eq!(nearest.passes[0].outputs, ["OUTPUT"]);
    }

    #[test]
    fn parses_parameters_and_compute_pass_sizes() {
        let source = r"
//!MAGPIE EFFECT
//!VERSION 4
//!SORT_NAME Test_Effect
//!PARAMETER
//!LABEL Sharpness
//!DEFAULT 0.4
//!MIN 0
//!MAX 1
//!STEP 0.01
float sharpness;
//!TEXTURE
Texture2D INPUT;
//!TEXTURE
Texture2D OUTPUT;
//!TEXTURE
//!SOURCE Lut.dds
//!FORMAT R16G16B16A16_FLOAT
Texture2D LUT;
//!COMMON
float CommonValue() { return 1; }
//!PASS 1
//!DESC Setup
//!IN INPUT
//!OUT OUTPUT
//!BLOCK_SIZE 16, 8
//!NUM_THREADS 64, 4
void Pass1(uint2 blockStart, uint3 threadId) {}
";

        let effect = parse_magpiefx(source).unwrap();

        assert_eq!(effect.sort_name.as_deref(), Some("Test_Effect"));
        assert_eq!(effect.parameters[0].symbol, "sharpness");
        assert_eq!(effect.parameters[0].label.as_deref(), Some("Sharpness"));
        assert_eq!(effect.textures[2].source.as_deref(), Some("Lut.dds"));
        assert!(effect.hlsl_source.contains("float CommonValue()"));
        assert!(effect.common_source.contains("float CommonValue()"));
        assert!(effect.passes[0].source.contains("void Pass1"));
        assert!(!effect.passes[0].source.contains("Texture2D INPUT"));
        assert_eq!(effect.passes[0].description.as_deref(), Some("Setup"));
        assert_eq!(effect.passes[0].block_size.as_deref(), Some(&[16, 8][..]));
        assert_eq!(effect.passes[0].num_threads.as_deref(), Some(&[64, 4][..]));
    }

    #[test]
    fn parses_effect_from_file() {
        let path = std::env::temp_dir().join(format!(
            "borderless-magpiefx-test-{}.hlsl",
            std::process::id()
        ));
        std::fs::write(&path, NEAREST).unwrap();

        let effect = parse_magpiefx_file(&path).unwrap();

        let _ = std::fs::remove_file(&path);
        assert_eq!(effect.version, 4);
    }
}
