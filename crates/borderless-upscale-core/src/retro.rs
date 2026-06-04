// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{
    CaptureBackend, CompatibilityMode, InputBackend, LegacyPresentationApi,
    LegacyPresentationStrategy, RendererBackend, ScalingBackend, ScalingPipeline, UpscaleError,
    UpscaleResult,
};
use borderless_core::{PhysicalRect, ScaleFactor};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetroCompatibilityRequest {
    pub profile: LegacyCompatibilityProfile,
    pub scaling: ScalingBackend,
    pub renderer: RendererBackend,
    pub source_rect: PhysicalRect,
    pub output_rect: PhysicalRect,
}

impl RetroCompatibilityRequest {
    #[must_use]
    pub const fn new(
        profile: LegacyCompatibilityProfile,
        source_rect: PhysicalRect,
        output_rect: PhysicalRect,
    ) -> Self {
        Self {
            profile,
            scaling: ScalingBackend::Integer,
            renderer: RendererBackend::WgpuDx12,
            source_rect,
            output_rect,
        }
    }

    #[must_use]
    pub const fn with_scaling(mut self, scaling: ScalingBackend) -> Self {
        self.scaling = scaling;
        self
    }

    #[must_use]
    pub const fn with_renderer(mut self, renderer: RendererBackend) -> Self {
        self.renderer = renderer;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RetroCompatibilityPlan {
    pub profile: LegacyCompatibilityProfile,
    pub pipeline: ScalingPipeline,
    pub requirements: Vec<RetroCompatibilityRequirement>,
}

impl RetroCompatibilityPlan {
    pub fn from_request(request: RetroCompatibilityRequest) -> UpscaleResult<Self> {
        request.profile.validate()?;
        request.profile.validate_retro_invariants()?;
        let pipeline = ScalingPipeline {
            compatibility_mode: compatibility_mode(request.profile.presentation_strategy),
            capture: capture_backend(
                request.profile.presentation_api,
                request.profile.presentation_strategy,
            ),
            scaling: request.scaling,
            input: request.profile.input_backend,
            renderer: request.renderer,
            source_scale: ScaleFactor::default(),
            source_rect: request.source_rect,
            output_rect: request.output_rect,
        };
        pipeline.validate()?;
        Ok(Self {
            requirements: requirements_for_profile(&request.profile),
            profile: request.profile,
            pipeline,
        })
    }

    #[must_use]
    pub fn requires_wrapper(&self) -> bool {
        self.requirements
            .contains(&RetroCompatibilityRequirement::ApiWrapper)
    }

    #[must_use]
    pub fn requires_proxy_surface(&self) -> bool {
        self.requirements
            .contains(&RetroCompatibilityRequirement::ProxySurface)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directdraw_recommendation_uses_wrapper_boundary() {
        let rect = PhysicalRect::new(0, 0, 640, 480).unwrap();
        let profile =
            LegacyCompatibilityProfile::recommended_for_api(LegacyPresentationApi::DirectDraw);

        let plan = profile.plan(rect, rect).unwrap();

        assert_eq!(
            plan.pipeline.compatibility_mode,
            CompatibilityMode::LegacyWrapper
        );
        assert_eq!(plan.pipeline.capture, CaptureBackend::DwmSharedSurface);
        assert_eq!(plan.pipeline.input, InputBackend::DirectInputShim);
        assert!(plan.requires_wrapper());
        assert!(
            plan.requirements
                .contains(&RetroCompatibilityRequirement::PaletteSync)
        );
    }

    #[test]
    fn direct3d9_recommendation_uses_proxy_surface() {
        let rect = PhysicalRect::new(0, 0, 1280, 720).unwrap();
        let profile =
            LegacyCompatibilityProfile::recommended_for_api(LegacyPresentationApi::Direct3d9);

        let plan = profile.plan(rect, rect).unwrap();

        assert_eq!(
            plan.pipeline.compatibility_mode,
            CompatibilityMode::ProxyPresentation
        );
        assert_eq!(plan.pipeline.capture, CaptureBackend::DesktopDuplication);
        assert_eq!(plan.pipeline.input, InputBackend::RawInputShim);
        assert!(plan.requires_proxy_surface());
    }

    #[test]
    fn rejects_palette_sync_for_non_palette_api() {
        let rect = PhysicalRect::new(0, 0, 640, 480).unwrap();
        let profile = LegacyCompatibilityProfile {
            presentation_api: LegacyPresentationApi::Direct3d9,
            requires_palette_sync: true,
            ..LegacyCompatibilityProfile::recommended_for_api(LegacyPresentationApi::Direct3d9)
        };

        assert!(matches!(
            profile.plan(rect, rect),
            Err(UpscaleError::InvalidPipeline(_))
        ));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetroCompatibilityRequirement {
    PhysicalPixelCoordinates,
    PreserveAspectRatio,
    ProxySurface,
    ApiWrapper,
    InputRemap,
    PaletteSync,
    SoftwareCursor,
    RuntimeRestore,
}

pub use crate::LegacyCompatibilityProfile;

impl LegacyCompatibilityProfile {
    #[must_use]
    pub const fn recommended_for_api(api: LegacyPresentationApi) -> Self {
        match api {
            LegacyPresentationApi::None => Self::inactive(),
            LegacyPresentationApi::Gdi => Self {
                presentation_api: api,
                presentation_strategy: LegacyPresentationStrategy::ProxyPresentation,
                input_backend: InputBackend::WindowMessageRemap,
                preserve_aspect_ratio: true,
                requires_palette_sync: true,
                force_software_cursor: true,
            },
            LegacyPresentationApi::DirectDraw => Self {
                presentation_api: api,
                presentation_strategy: LegacyPresentationStrategy::WrappedPresentation,
                input_backend: InputBackend::DirectInputShim,
                preserve_aspect_ratio: true,
                requires_palette_sync: true,
                force_software_cursor: true,
            },
            LegacyPresentationApi::Direct3d8 => Self {
                presentation_api: api,
                presentation_strategy: LegacyPresentationStrategy::WrappedPresentation,
                input_backend: InputBackend::DirectInputShim,
                preserve_aspect_ratio: true,
                requires_palette_sync: false,
                force_software_cursor: false,
            },
            LegacyPresentationApi::Glide => Self {
                presentation_api: api,
                presentation_strategy: LegacyPresentationStrategy::WrappedPresentation,
                input_backend: InputBackend::DirectInputShim,
                preserve_aspect_ratio: true,
                requires_palette_sync: false,
                force_software_cursor: true,
            },
            LegacyPresentationApi::Direct3d9 => Self {
                presentation_api: api,
                presentation_strategy: LegacyPresentationStrategy::ProxyPresentation,
                input_backend: InputBackend::RawInputShim,
                preserve_aspect_ratio: true,
                requires_palette_sync: false,
                force_software_cursor: false,
            },
        }
    }

    pub fn plan(
        self,
        source_rect: PhysicalRect,
        output_rect: PhysicalRect,
    ) -> UpscaleResult<RetroCompatibilityPlan> {
        RetroCompatibilityPlan::from_request(RetroCompatibilityRequest::new(
            self,
            source_rect,
            output_rect,
        ))
    }
}

fn compatibility_mode(strategy: LegacyPresentationStrategy) -> CompatibilityMode {
    match strategy {
        LegacyPresentationStrategy::None | LegacyPresentationStrategy::BorderlessWindow => {
            CompatibilityMode::NativeBorderless
        }
        LegacyPresentationStrategy::ProxyPresentation => CompatibilityMode::ProxyPresentation,
        LegacyPresentationStrategy::WrappedPresentation => CompatibilityMode::LegacyWrapper,
    }
}

fn capture_backend(
    api: LegacyPresentationApi,
    strategy: LegacyPresentationStrategy,
) -> CaptureBackend {
    match (api, strategy) {
        (_, LegacyPresentationStrategy::WrappedPresentation) => CaptureBackend::DwmSharedSurface,
        (LegacyPresentationApi::Gdi, _) => CaptureBackend::Gdi,
        (_, LegacyPresentationStrategy::ProxyPresentation) => CaptureBackend::DesktopDuplication,
        _ => CaptureBackend::GraphicsCapture,
    }
}

fn requirements_for_profile(
    profile: &LegacyCompatibilityProfile,
) -> Vec<RetroCompatibilityRequirement> {
    let mut requirements = vec![
        RetroCompatibilityRequirement::PhysicalPixelCoordinates,
        RetroCompatibilityRequirement::RuntimeRestore,
    ];
    if profile.preserve_aspect_ratio {
        requirements.push(RetroCompatibilityRequirement::PreserveAspectRatio);
    }
    if profile.presentation_strategy == LegacyPresentationStrategy::ProxyPresentation {
        requirements.push(RetroCompatibilityRequirement::ProxySurface);
    }
    if profile.presentation_strategy == LegacyPresentationStrategy::WrappedPresentation {
        requirements.push(RetroCompatibilityRequirement::ApiWrapper);
    }
    if profile.input_backend != InputBackend::NoRemap {
        requirements.push(RetroCompatibilityRequirement::InputRemap);
    }
    if profile.requires_palette_sync {
        requirements.push(RetroCompatibilityRequirement::PaletteSync);
    }
    if profile.force_software_cursor {
        requirements.push(RetroCompatibilityRequirement::SoftwareCursor);
    }
    requirements
}

impl LegacyCompatibilityProfile {
    pub(crate) fn validate_retro_invariants(&self) -> UpscaleResult<()> {
        if self.requires_palette_sync
            && !matches!(
                self.presentation_api,
                LegacyPresentationApi::Gdi | LegacyPresentationApi::DirectDraw
            )
        {
            return Err(UpscaleError::InvalidPipeline(
                "palette sync is only valid for GDI or DirectDraw profiles".to_owned(),
            ));
        }
        Ok(())
    }
}
