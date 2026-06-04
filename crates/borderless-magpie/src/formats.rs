// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: Copyright (c) Xu and Magpie contributors
// SPDX-FileCopyrightText: Copyright (c) Borderless Oxide contributors
// Magpie-compatible texture format descriptors for the GPL port:
// https://github.com/Blinue/Magpie

use crate::plan::MagpieTextureFormat;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieTextureFormatDescriptor {
    pub format: MagpieTextureFormat,
    pub dxgi_format: Option<u32>,
    pub bytes_per_pixel: Option<u32>,
    pub channels: Option<u8>,
    pub component: MagpieTextureComponent,
    pub normalized: bool,
    pub signed: bool,
    pub srgb: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MagpieTextureComponent {
    #[default]
    Unknown,
    Unorm,
    Snorm,
    Float,
}

impl MagpieTextureFormat {
    #[must_use]
    pub fn descriptor(&self) -> MagpieTextureFormatDescriptor {
        let component = component(self);

        MagpieTextureFormatDescriptor {
            format: self.clone(),
            dxgi_format: dxgi_format(self),
            bytes_per_pixel: bytes_per_pixel(self),
            channels: channels(self),
            component,
            normalized: matches!(
                component,
                MagpieTextureComponent::Unorm | MagpieTextureComponent::Snorm
            ),
            signed: signed(self),
            srgb: srgb(self),
        }
    }

    #[must_use]
    pub fn is_renderer_known(&self) -> bool {
        !matches!(self, Self::Unknown(_)) && self.descriptor().bytes_per_pixel.is_some()
    }
}

fn dxgi_format(format: &MagpieTextureFormat) -> Option<u32> {
    match format {
        MagpieTextureFormat::R8Unorm => Some(61),
        MagpieTextureFormat::R8g8Unorm => Some(49),
        MagpieTextureFormat::R8g8b8a8Unorm => Some(28),
        MagpieTextureFormat::R8g8b8a8UnormSrgb => Some(29),
        MagpieTextureFormat::R8g8b8a8Snorm => Some(31),
        MagpieTextureFormat::R16Float => Some(54),
        MagpieTextureFormat::R16g16Float => Some(34),
        MagpieTextureFormat::R16g16b16a16Float => Some(10),
        MagpieTextureFormat::R32Float => Some(41),
        MagpieTextureFormat::R32g32b32a32Float => Some(2),
        MagpieTextureFormat::Dxgi(value) => Some(*value),
        MagpieTextureFormat::Unknown(_) => None,
    }
}

fn bytes_per_pixel(format: &MagpieTextureFormat) -> Option<u32> {
    match format {
        MagpieTextureFormat::R8Unorm => Some(1),
        MagpieTextureFormat::R8g8Unorm | MagpieTextureFormat::R16Float => Some(2),
        MagpieTextureFormat::R8g8b8a8Unorm
        | MagpieTextureFormat::R8g8b8a8UnormSrgb
        | MagpieTextureFormat::R8g8b8a8Snorm
        | MagpieTextureFormat::R16g16Float
        | MagpieTextureFormat::R32Float => Some(4),
        MagpieTextureFormat::R16g16b16a16Float => Some(8),
        MagpieTextureFormat::R32g32b32a32Float => Some(16),
        MagpieTextureFormat::Dxgi(_) | MagpieTextureFormat::Unknown(_) => None,
    }
}

fn channels(format: &MagpieTextureFormat) -> Option<u8> {
    match format {
        MagpieTextureFormat::R8Unorm
        | MagpieTextureFormat::R16Float
        | MagpieTextureFormat::R32Float => Some(1),
        MagpieTextureFormat::R8g8Unorm | MagpieTextureFormat::R16g16Float => Some(2),
        MagpieTextureFormat::R8g8b8a8Unorm
        | MagpieTextureFormat::R8g8b8a8UnormSrgb
        | MagpieTextureFormat::R8g8b8a8Snorm
        | MagpieTextureFormat::R16g16b16a16Float
        | MagpieTextureFormat::R32g32b32a32Float => Some(4),
        MagpieTextureFormat::Dxgi(_) | MagpieTextureFormat::Unknown(_) => None,
    }
}

fn component(format: &MagpieTextureFormat) -> MagpieTextureComponent {
    match format {
        MagpieTextureFormat::R8Unorm
        | MagpieTextureFormat::R8g8Unorm
        | MagpieTextureFormat::R8g8b8a8Unorm
        | MagpieTextureFormat::R8g8b8a8UnormSrgb => MagpieTextureComponent::Unorm,
        MagpieTextureFormat::R8g8b8a8Snorm => MagpieTextureComponent::Snorm,
        MagpieTextureFormat::R16Float
        | MagpieTextureFormat::R16g16Float
        | MagpieTextureFormat::R16g16b16a16Float
        | MagpieTextureFormat::R32Float
        | MagpieTextureFormat::R32g32b32a32Float => MagpieTextureComponent::Float,
        MagpieTextureFormat::Dxgi(_) | MagpieTextureFormat::Unknown(_) => {
            MagpieTextureComponent::Unknown
        }
    }
}

fn signed(format: &MagpieTextureFormat) -> bool {
    matches!(
        format,
        MagpieTextureFormat::R8g8b8a8Snorm
            | MagpieTextureFormat::R16Float
            | MagpieTextureFormat::R16g16Float
            | MagpieTextureFormat::R16g16b16a16Float
            | MagpieTextureFormat::R32Float
            | MagpieTextureFormat::R32g32b32a32Float
    )
}

fn srgb(format: &MagpieTextureFormat) -> bool {
    matches!(format, MagpieTextureFormat::R8g8b8a8UnormSrgb)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describes_known_dxgi_formats() {
        let rgba8 = MagpieTextureFormat::R8g8b8a8Unorm.descriptor();
        assert_eq!(rgba8.dxgi_format, Some(28));
        assert_eq!(rgba8.bytes_per_pixel, Some(4));
        assert_eq!(rgba8.channels, Some(4));
        assert_eq!(rgba8.component, MagpieTextureComponent::Unorm);
        assert!(rgba8.normalized);
        assert!(!rgba8.srgb);

        let fp16 = MagpieTextureFormat::R16g16b16a16Float.descriptor();
        assert_eq!(fp16.dxgi_format, Some(10));
        assert_eq!(fp16.bytes_per_pixel, Some(8));
        assert_eq!(fp16.component, MagpieTextureComponent::Float);
    }

    #[test]
    fn preserves_unknown_dxgi_without_claiming_layout() {
        let descriptor = MagpieTextureFormat::Dxgi(98).descriptor();

        assert_eq!(descriptor.dxgi_format, Some(98));
        assert_eq!(descriptor.bytes_per_pixel, None);
        assert!(!MagpieTextureFormat::Dxgi(98).is_renderer_known());
    }
}
