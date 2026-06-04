// SPDX-License-Identifier: GPL-3.0-or-later

use borderless_upscale_core::{ScalingBackend, UpscaleError, UpscaleResult};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieEffect {
    pub name: String,
    pub source: EffectSource,
    pub passes: Vec<EffectPass>,
}

impl MagpieEffect {
    pub fn validate(&self) -> UpscaleResult<()> {
        if self.name.trim().is_empty() {
            return Err(UpscaleError::InvalidPipeline(
                "effect name must not be empty".to_owned(),
            ));
        }
        if self.passes.is_empty() {
            return Err(UpscaleError::InvalidPipeline(format!(
                "effect {} must contain at least one pass",
                self.name
            )));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EffectSource {
    BuiltIn { id: String },
    HlslFile { relative_path: String },
    HlslSource { source: String },
    WgslFile { relative_path: String },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectPassStyle {
    PixelShader,
    #[default]
    Compute,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectPass {
    pub name: String,
    pub description: Option<String>,
    pub scale_num: u32,
    pub scale_den: u32,
    pub style: EffectPassStyle,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub block_size: Option<Vec<u32>>,
    pub num_threads: Option<Vec<u32>>,
}

impl EffectPass {
    #[must_use]
    pub fn new(name: impl Into<String>, scale_num: u32, scale_den: u32) -> Self {
        Self {
            name: name.into(),
            description: None,
            scale_num: scale_num.max(1),
            scale_den: scale_den.max(1),
            style: EffectPassStyle::Compute,
            inputs: Vec::new(),
            outputs: Vec::new(),
            block_size: None,
            num_threads: None,
        }
    }

    #[must_use]
    pub fn with_shader_metadata(
        mut self,
        description: Option<String>,
        style: EffectPassStyle,
        inputs: impl IntoIterator<Item = String>,
        outputs: impl IntoIterator<Item = String>,
        block_size: Option<Vec<u32>>,
        num_threads: Option<Vec<u32>>,
    ) -> Self {
        self.description = description;
        self.style = style;
        self.inputs = inputs.into_iter().collect();
        self.outputs = outputs.into_iter().collect();
        self.block_size = block_size;
        self.num_threads = num_threads;
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectGraph {
    effects: Vec<MagpieEffect>,
}

impl EffectGraph {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            effects: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_effect(mut self, effect: MagpieEffect) -> Self {
        self.effects.push(effect);
        self
    }

    #[must_use]
    pub fn effects(&self) -> &[MagpieEffect] {
        &self.effects
    }

    pub fn validate(&self) -> UpscaleResult<()> {
        if self.effects.is_empty() {
            return Err(UpscaleError::InvalidPipeline(
                "MagpieFX graph must contain at least one effect".to_owned(),
            ));
        }
        self.effects.iter().try_for_each(MagpieEffect::validate)
    }

    #[must_use]
    pub const fn scaling_backend(&self) -> ScalingBackend {
        ScalingBackend::MagpieFxCompatible
    }
}

#[cfg(feature = "shader-assets")]
#[must_use]
pub fn shader_assets_enabled() -> bool {
    true
}

#[cfg(not(feature = "shader-assets"))]
#[must_use]
pub fn shader_assets_enabled() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_graph_is_invalid() {
        assert!(EffectGraph::new().validate().is_err());
    }
}
