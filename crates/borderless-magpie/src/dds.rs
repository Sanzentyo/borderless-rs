// SPDX-License-Identifier: GPL-3.0-or-later
// DDS source texture metadata support for the Magpie-compatible GPL port:
// https://github.com/Blinue/Magpie

use crate::plan::MagpieTextureFormat;
use borderless_upscale_core::{FrameSize, UpscaleError, UpscaleResult};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::Path;

const DDS_MAGIC: &[u8; 4] = b"DDS ";
const HEADER_SIZE: usize = 124;
const MIN_DDS_SIZE: usize = 4 + HEADER_SIZE;
const DDS_HEADER_SIZE_FIELD: u32 = 124;
const DDPF_FOURCC: u32 = 0x0000_0004;
const DX10_FOURCC: u32 = u32::from_le_bytes(*b"DX10");

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DdsMetadata {
    pub size: FrameSize,
    pub format: Option<MagpieTextureFormat>,
}

pub fn read_dds_metadata(path: impl AsRef<Path>) -> UpscaleResult<DdsMetadata> {
    let path = path.as_ref();
    let mut file = std::fs::File::open(path).map_err(|err| {
        UpscaleError::BackendUnavailable(format!("failed to open DDS {}: {err}", path.display()))
    })?;
    let mut header = [0_u8; MIN_DDS_SIZE + 20];
    let bytes_read = file.read(&mut header).map_err(|err| {
        UpscaleError::BackendUnavailable(format!("failed to read DDS {}: {err}", path.display()))
    })?;
    parse_dds_metadata(&header[..bytes_read])
}

pub fn parse_dds_metadata(bytes: &[u8]) -> UpscaleResult<DdsMetadata> {
    if bytes.len() < MIN_DDS_SIZE {
        return Err(invalid_pipeline("DDS header is truncated"));
    }
    if &bytes[..4] != DDS_MAGIC {
        return Err(invalid_pipeline("DDS magic is missing"));
    }
    let header = &bytes[4..MIN_DDS_SIZE];
    let header_size = read_u32(header, 0)?;
    if header_size != DDS_HEADER_SIZE_FIELD {
        return Err(invalid_pipeline(format!(
            "unexpected DDS header size: {header_size}"
        )));
    }

    let height = read_u32(header, 8)?;
    let width = read_u32(header, 12)?;
    let size = FrameSize::new(width, height)
        .ok_or_else(|| invalid_pipeline("DDS width/height must be non-zero"))?;
    let pixel_format_flags = read_u32(header, 76)?;
    let fourcc = read_u32(header, 80)?;
    let format = if pixel_format_flags & DDPF_FOURCC != 0 && fourcc == DX10_FOURCC {
        parse_dx10_format(bytes)
    } else {
        None
    };

    Ok(DdsMetadata { size, format })
}

fn parse_dx10_format(bytes: &[u8]) -> Option<MagpieTextureFormat> {
    if bytes.len() < MIN_DDS_SIZE + 20 {
        return None;
    }
    let dxgi_format = read_u32_lossy(bytes, MIN_DDS_SIZE);
    match dxgi_format {
        2 => Some(MagpieTextureFormat::R32g32b32a32Float),
        10 => Some(MagpieTextureFormat::R16g16b16a16Float),
        28 => Some(MagpieTextureFormat::R8g8b8a8Unorm),
        29 => Some(MagpieTextureFormat::R8g8b8a8UnormSrgb),
        31 => Some(MagpieTextureFormat::R8g8b8a8Snorm),
        34 => Some(MagpieTextureFormat::R16g16Float),
        41 => Some(MagpieTextureFormat::R32Float),
        49 => Some(MagpieTextureFormat::R8g8Unorm),
        54 => Some(MagpieTextureFormat::R16Float),
        61 => Some(MagpieTextureFormat::R8Unorm),
        _ => Some(MagpieTextureFormat::Dxgi(dxgi_format)),
    }
}

fn read_u32(bytes: &[u8], offset: usize) -> UpscaleResult<u32> {
    bytes
        .get(offset..offset + 4)
        .and_then(|value| value.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| invalid_pipeline("DDS header is truncated"))
}

fn read_u32_lossy(bytes: &[u8], offset: usize) -> u32 {
    bytes
        .get(offset..offset + 4)
        .and_then(|value| value.try_into().ok())
        .map(u32::from_le_bytes)
        .unwrap_or_default()
}

fn invalid_pipeline(message: impl Into<String>) -> UpscaleError {
    UpscaleError::InvalidPipeline(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_dx10_dds_metadata() {
        let bytes = fake_dds_dx10(320, 240, 10);
        let metadata = parse_dds_metadata(&bytes).unwrap();

        assert_eq!(metadata.size, FrameSize::new(320, 240).unwrap());
        assert_eq!(
            metadata.format,
            Some(MagpieTextureFormat::R16g16b16a16Float)
        );
    }

    #[test]
    fn rejects_non_dds_bytes() {
        assert!(matches!(
            parse_dds_metadata(b"not a dds"),
            Err(UpscaleError::InvalidPipeline(_))
        ));
    }

    fn fake_dds_dx10(width: u32, height: u32, dxgi_format: u32) -> Vec<u8> {
        let mut bytes = vec![0_u8; MIN_DDS_SIZE + 20];
        bytes[..4].copy_from_slice(DDS_MAGIC);
        write_u32(&mut bytes, 4, DDS_HEADER_SIZE_FIELD);
        write_u32(&mut bytes, 12, height);
        write_u32(&mut bytes, 16, width);
        write_u32(&mut bytes, 76, 32);
        write_u32(&mut bytes, 80, DDPF_FOURCC);
        write_u32(&mut bytes, 84, DX10_FOURCC);
        write_u32(&mut bytes, MIN_DDS_SIZE, dxgi_format);
        bytes
    }

    fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
}
