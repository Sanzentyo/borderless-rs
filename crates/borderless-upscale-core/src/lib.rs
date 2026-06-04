// SPDX-License-Identifier: MIT OR Apache-2.0

#![forbid(unsafe_code)]

use borderless_core::{PhysicalRect, ScaleFactor};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type UpscaleResult<T> = Result<T, UpscaleError>;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum UpscaleError {
    #[error("backend is disabled: {0}")]
    BackendDisabled(&'static str),
    #[error("backend is unavailable: {0}")]
    BackendUnavailable(String),
    #[error("invalid pipeline: {0}")]
    InvalidPipeline(String),
    #[error("frame operation failed: {0}")]
    Frame(String),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityMode {
    #[default]
    NativeBorderless,
    ProxyPresentation,
    LegacyWrapper,
    HookedPresentation,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureBackend {
    #[default]
    GraphicsCapture,
    DesktopDuplication,
    DwmSharedSurface,
    Gdi,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScalingBackend {
    #[default]
    Integer,
    Lanczos,
    Fsr1,
    Anime4k,
    MagpieFxCompatible,
    DirectMl,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputBackend {
    #[default]
    NoRemap,
    ClipCursor,
    WindowMessageRemap,
    DirectInputShim,
    RawInputShim,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacyPresentationApi {
    #[default]
    None,
    Gdi,
    DirectDraw,
    Direct3d8,
    Direct3d9,
    Glide,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacyPresentationStrategy {
    #[default]
    None,
    BorderlessWindow,
    ProxyPresentation,
    WrappedPresentation,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegacyCompatibilityProfile {
    pub presentation_api: LegacyPresentationApi,
    pub presentation_strategy: LegacyPresentationStrategy,
    pub input_backend: InputBackend,
    pub preserve_aspect_ratio: bool,
    pub requires_palette_sync: bool,
    pub force_software_cursor: bool,
}

impl LegacyCompatibilityProfile {
    #[must_use]
    pub const fn inactive() -> Self {
        Self {
            presentation_api: LegacyPresentationApi::None,
            presentation_strategy: LegacyPresentationStrategy::None,
            input_backend: InputBackend::NoRemap,
            preserve_aspect_ratio: false,
            requires_palette_sync: false,
            force_software_cursor: false,
        }
    }

    pub fn validate(&self) -> UpscaleResult<()> {
        if matches!(
            self.presentation_strategy,
            LegacyPresentationStrategy::WrappedPresentation
        ) && matches!(self.presentation_api, LegacyPresentationApi::None)
        {
            return Err(UpscaleError::InvalidPipeline(
                "wrapped legacy presentation requires a concrete presentation API".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RendererBackend {
    NativeDirect3d,
    #[default]
    WgpuDx12,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameFormat {
    #[default]
    Bgra8Unorm,
    Rgba8Unorm,
    Rgba16Float,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameSize {
    pub width: u32,
    pub height: u32,
}

impl FrameSize {
    #[must_use]
    pub const fn new(width: u32, height: u32) -> Option<Self> {
        if width == 0 || height == 0 {
            None
        } else {
            Some(Self { width, height })
        }
    }

    #[must_use]
    pub fn pixels(self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameDescriptor {
    pub size: FrameSize,
    pub format: FrameFormat,
    pub source_rect: PhysicalRect,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScalingPipeline {
    pub compatibility_mode: CompatibilityMode,
    pub capture: CaptureBackend,
    pub scaling: ScalingBackend,
    pub input: InputBackend,
    pub renderer: RendererBackend,
    pub source_scale: ScaleFactor,
    pub source_rect: PhysicalRect,
    pub output_rect: PhysicalRect,
}

impl ScalingPipeline {
    #[must_use]
    pub fn proxy_presentation(
        capture: CaptureBackend,
        scaling: ScalingBackend,
        input: InputBackend,
        renderer: RendererBackend,
        source_rect: PhysicalRect,
        output_rect: PhysicalRect,
    ) -> Self {
        Self {
            compatibility_mode: CompatibilityMode::ProxyPresentation,
            capture,
            scaling,
            input,
            renderer,
            source_scale: ScaleFactor::default(),
            source_rect,
            output_rect,
        }
    }

    pub fn validate(&self) -> UpscaleResult<()> {
        if matches!(
            self.compatibility_mode,
            CompatibilityMode::LegacyWrapper | CompatibilityMode::HookedPresentation
        ) && matches!(self.input, InputBackend::NoRemap)
        {
            return Err(UpscaleError::InvalidPipeline(
                "hooked/legacy modes require an explicit input backend".to_owned(),
            ));
        }
        Ok(())
    }
}

pub trait FrameSource {
    fn descriptor(&self) -> UpscaleResult<FrameDescriptor>;
    fn acquire_next_frame(&mut self) -> UpscaleResult<FrameDescriptor>;
}

pub trait FrameScaler {
    fn scaling_backend(&self) -> ScalingBackend;
    fn scale_frame(
        &mut self,
        source: FrameDescriptor,
        output: PhysicalRect,
    ) -> UpscaleResult<FrameDescriptor>;
}

pub trait FramePresenter {
    fn renderer_backend(&self) -> RendererBackend;
    fn present(&mut self, frame: FrameDescriptor) -> UpscaleResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_pipeline_accepts_no_remap() {
        let rect = PhysicalRect::new(0, 0, 640, 480).unwrap();
        let pipeline = ScalingPipeline::proxy_presentation(
            CaptureBackend::GraphicsCapture,
            ScalingBackend::Integer,
            InputBackend::NoRemap,
            RendererBackend::WgpuDx12,
            rect,
            rect,
        );

        assert_eq!(pipeline.validate(), Ok(()));
    }

    #[test]
    fn hooked_pipeline_requires_input_backend() {
        let rect = PhysicalRect::new(0, 0, 640, 480).unwrap();
        let pipeline = ScalingPipeline {
            compatibility_mode: CompatibilityMode::HookedPresentation,
            capture: CaptureBackend::GraphicsCapture,
            scaling: ScalingBackend::MagpieFxCompatible,
            input: InputBackend::NoRemap,
            renderer: RendererBackend::NativeDirect3d,
            source_scale: ScaleFactor::default(),
            source_rect: rect,
            output_rect: rect,
        };

        assert!(matches!(
            pipeline.validate(),
            Err(UpscaleError::InvalidPipeline(_))
        ));
    }

    #[test]
    fn wrapped_legacy_profile_requires_api() {
        let profile = LegacyCompatibilityProfile {
            presentation_strategy: LegacyPresentationStrategy::WrappedPresentation,
            ..LegacyCompatibilityProfile::inactive()
        };

        assert!(matches!(
            profile.validate(),
            Err(UpscaleError::InvalidPipeline(_))
        ));
    }

    #[test]
    fn directdraw_legacy_profile_accepts_wrapped_presentation() {
        let profile = LegacyCompatibilityProfile {
            presentation_api: LegacyPresentationApi::DirectDraw,
            presentation_strategy: LegacyPresentationStrategy::WrappedPresentation,
            input_backend: InputBackend::DirectInputShim,
            preserve_aspect_ratio: true,
            requires_palette_sync: true,
            force_software_cursor: true,
        };

        assert_eq!(profile.validate(), Ok(()));
    }
}
