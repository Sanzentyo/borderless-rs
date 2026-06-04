// SPDX-License-Identifier: GPL-3.0-or-later

use borderless_upscale_core::{
    FrameDescriptor, FramePresenter, RendererBackend, UpscaleError, UpscaleResult,
};

#[derive(Clone, Debug, Default)]
pub struct WgpuDx12Renderer {
    initialized: bool,
    #[cfg(feature = "dx12")]
    backend: wgpu::Backends,
}

impl WgpuDx12Renderer {
    pub fn new() -> UpscaleResult<Self> {
        #[cfg(feature = "dx12")]
        {
            Ok(Self {
                initialized: true,
                backend: wgpu::Backends::DX12,
            })
        }

        #[cfg(not(feature = "dx12"))]
        {
            Err(UpscaleError::BackendDisabled("wgpu dx12"))
        }
    }

    #[must_use]
    pub const fn is_initialized(&self) -> bool {
        self.initialized
    }

    #[cfg(feature = "dx12")]
    #[must_use]
    pub const fn backend(&self) -> wgpu::Backends {
        self.backend
    }
}

impl FramePresenter for WgpuDx12Renderer {
    fn renderer_backend(&self) -> RendererBackend {
        RendererBackend::WgpuDx12
    }

    fn present(&mut self, _frame: FrameDescriptor) -> UpscaleResult<()> {
        if self.initialized {
            Ok(())
        } else {
            Err(UpscaleError::BackendUnavailable(
                "wgpu dx12 renderer is not initialized".to_owned(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_renderer_reports_disabled_backend() {
        #[cfg(not(feature = "dx12"))]
        assert!(matches!(
            WgpuDx12Renderer::new(),
            Err(UpscaleError::BackendDisabled("wgpu dx12"))
        ));
    }
}
