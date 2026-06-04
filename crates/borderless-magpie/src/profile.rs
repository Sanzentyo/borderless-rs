// SPDX-License-Identifier: GPL-3.0-or-later

use crate::effect::{EffectGraph, EffectPass, EffectSource, MagpieEffect};
use borderless_core::PhysicalRect;
use borderless_upscale_core::{
    CaptureBackend, InputBackend, RendererBackend, ScalingBackend, ScalingPipeline,
};

#[must_use]
pub fn tsukihime_profile(source_rect: PhysicalRect, output_rect: PhysicalRect) -> ScalingPipeline {
    ScalingPipeline::proxy_presentation(
        CaptureBackend::GraphicsCapture,
        ScalingBackend::Integer,
        InputBackend::ClipCursor,
        RendererBackend::NativeDirect3d,
        source_rect,
        output_rect,
    )
}

#[must_use]
pub fn chaos_child_profile(
    source_rect: PhysicalRect,
    output_rect: PhysicalRect,
) -> ScalingPipeline {
    ScalingPipeline::proxy_presentation(
        CaptureBackend::GraphicsCapture,
        ScalingBackend::MagpieFxCompatible,
        InputBackend::WindowMessageRemap,
        RendererBackend::NativeDirect3d,
        source_rect,
        output_rect,
    )
}

#[must_use]
pub fn anime4k_graph() -> EffectGraph {
    EffectGraph::new().with_effect(MagpieEffect {
        name: "Anime4K-compatible upscale".to_owned(),
        source: EffectSource::BuiltIn {
            id: "anime4k".to_owned(),
        },
        passes: vec![EffectPass::new("upscale", 2, 1)],
    })
}
