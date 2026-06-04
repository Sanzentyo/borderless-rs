// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: Copyright (c) Xu and Magpie contributors
// SPDX-FileCopyrightText: Copyright (c) Borderless Oxide contributors
// Magpie-compatible source texture upload planning for the GPL port:
// https://github.com/Blinue/Magpie

use crate::allocation::{MagpieTextureAllocation, MagpieTextureAllocationPlan, MagpieTextureUsage};
use crate::formats::MagpieTextureFormatDescriptor;
use crate::plan::MagpieTextureRole;
use borderless_upscale_core::{FrameSize, UpscaleError, UpscaleResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const DDS_MAGIC: &[u8; 4] = b"DDS ";
const DDS_HEADER_BYTES: usize = 128;
const DDS_DX10_HEADER_BYTES: usize = 20;
const DDPF_FOURCC: u32 = 0x0000_0004;
const DX10_FOURCC: u32 = u32::from_le_bytes(*b"DX10");

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieSourceUploadPlan {
    pub uploads: Vec<MagpieSourceUpload>,
}

impl MagpieSourceUploadPlan {
    pub fn from_allocations(allocations: &MagpieTextureAllocationPlan) -> UpscaleResult<Self> {
        let uploads = allocations
            .textures
            .iter()
            .filter(|texture| texture.role == MagpieTextureRole::SourceAsset)
            .map(source_upload)
            .collect::<UpscaleResult<Vec<_>>>()?;
        Ok(Self { uploads })
    }

    #[must_use]
    pub fn upload(&self, texture_name: &str) -> Option<&MagpieSourceUpload> {
        self.uploads
            .iter()
            .find(|upload| upload.texture_name == texture_name)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieSourceUpload {
    pub texture_name: String,
    pub path: PathBuf,
    pub size: FrameSize,
    pub format: MagpieTextureFormatDescriptor,
    pub data_offset: usize,
    pub row_pitch: u32,
    pub byte_len: u64,
}

fn source_upload(texture: &MagpieTextureAllocation) -> UpscaleResult<MagpieSourceUpload> {
    if !texture.usage.contains(&MagpieTextureUsage::SourceUpload) {
        return Err(invalid_pipeline(format!(
            "texture {} is not a source upload texture",
            texture.name
        )));
    }
    let path = texture
        .source_path
        .clone()
        .ok_or_else(|| invalid_pipeline(format!("source texture {} has no path", texture.name)))?;
    let data_offset = dds_data_offset(&std::fs::read(&path).map_err(|err| {
        UpscaleError::BackendUnavailable(format!(
            "failed to read source texture {}: {err}",
            path.display()
        ))
    })?)?;
    let bytes_per_pixel = texture.format_descriptor.bytes_per_pixel.ok_or_else(|| {
        invalid_pipeline(format!(
            "source texture {} has unsupported upload format",
            texture.name
        ))
    })?;
    let row_pitch = texture
        .size
        .width
        .checked_mul(bytes_per_pixel)
        .ok_or_else(|| {
            invalid_pipeline(format!(
                "source texture {} row pitch overflowed",
                texture.name
            ))
        })?;
    let byte_len = u64::from(row_pitch)
        .checked_mul(u64::from(texture.size.height))
        .ok_or_else(|| {
            invalid_pipeline(format!(
                "source texture {} byte size overflowed",
                texture.name
            ))
        })?;

    Ok(MagpieSourceUpload {
        texture_name: texture.name.clone(),
        path,
        size: texture.size,
        format: texture.format_descriptor.clone(),
        data_offset,
        row_pitch,
        byte_len,
    })
}

fn dds_data_offset(bytes: &[u8]) -> UpscaleResult<usize> {
    if bytes.len() < DDS_HEADER_BYTES {
        return Err(invalid_pipeline("DDS source texture is truncated"));
    }
    if &bytes[..4] != DDS_MAGIC {
        return Err(invalid_pipeline("DDS source texture magic is missing"));
    }
    let header = &bytes[4..DDS_HEADER_BYTES];
    let flags = read_u32(header, 76)?;
    let fourcc = read_u32(header, 80)?;
    let offset = if flags & DDPF_FOURCC != 0 && fourcc == DX10_FOURCC {
        DDS_HEADER_BYTES + DDS_DX10_HEADER_BYTES
    } else {
        DDS_HEADER_BYTES
    };
    if bytes.len() < offset {
        return Err(invalid_pipeline("DDS source texture header is truncated"));
    }
    Ok(offset)
}

fn read_u32(bytes: &[u8], offset: usize) -> UpscaleResult<u32> {
    bytes
        .get(offset..offset + 4)
        .and_then(|value| value.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| invalid_pipeline("DDS source texture header is truncated"))
}

fn invalid_pipeline(message: impl Into<String>) -> UpscaleError {
    UpscaleError::InvalidPipeline(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::magpiefx::parse_magpiefx;
    use crate::plan::MagpieRenderPlan;
    use std::fs;
    use std::path::Path;

    #[test]
    fn plans_dx10_source_texture_upload() {
        let root = unique_temp_dir("source-upload");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("Lut.dds"), fake_dds_dx10(4, 2, 28, 32)).unwrap();
        let effect = parse_magpiefx(
            r"
//!MAGPIE EFFECT
//!VERSION 4
//!TEXTURE
Texture2D INPUT;
//!TEXTURE
//!SOURCE Lut.dds
Texture2D LUT;
//!TEXTURE
Texture2D OUTPUT;
//!PASS 1
//!IN INPUT, LUT
//!OUT OUTPUT
//!BLOCK_SIZE 8
//!NUM_THREADS 64
void Pass1(uint2 pos) { OUTPUT[pos] = INPUT[pos]; }
",
        )
        .unwrap();
        let input_size = FrameSize::new(320, 240).unwrap();
        let output_size = FrameSize::new(640, 480).unwrap();
        let render_plan = MagpieRenderPlan::from_effect_with_source_dir(
            &effect,
            Some(&root),
            input_size,
            output_size,
        )
        .unwrap();
        let allocations = MagpieTextureAllocationPlan::from_render_plan(&render_plan).unwrap();

        let uploads = MagpieSourceUploadPlan::from_allocations(&allocations).unwrap();
        let upload = uploads.upload("LUT").unwrap();

        assert_eq!(upload.data_offset, DDS_HEADER_BYTES + DDS_DX10_HEADER_BYTES);
        assert_eq!(upload.row_pitch, 16);
        assert_eq!(upload.byte_len, 32);
        assert_eq!(upload.format.dxgi_format, Some(28));
    }

    #[test]
    fn rejects_unsupported_upload_layout() {
        let root = unique_temp_dir("source-upload-unsupported");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("Lut.dds"), fake_dds_dx10(4, 2, 98, 32)).unwrap();
        let allocations = allocation_plan_for_source(&root);

        assert!(matches!(
            MagpieSourceUploadPlan::from_allocations(&allocations),
            Err(UpscaleError::InvalidPipeline(_))
        ));
    }

    fn allocation_plan_for_source(root: &Path) -> MagpieTextureAllocationPlan {
        let effect = parse_magpiefx(
            r"
//!MAGPIE EFFECT
//!VERSION 4
//!TEXTURE
Texture2D INPUT;
//!TEXTURE
//!SOURCE Lut.dds
Texture2D LUT;
//!TEXTURE
Texture2D OUTPUT;
//!PASS 1
//!IN INPUT, LUT
//!OUT OUTPUT
//!BLOCK_SIZE 8
//!NUM_THREADS 64
void Pass1(uint2 pos) { OUTPUT[pos] = INPUT[pos]; }
",
        )
        .unwrap();
        let input_size = FrameSize::new(320, 240).unwrap();
        let output_size = FrameSize::new(640, 480).unwrap();
        let render_plan = MagpieRenderPlan::from_effect_with_source_dir(
            &effect,
            Some(root),
            input_size,
            output_size,
        )
        .unwrap();
        MagpieTextureAllocationPlan::from_render_plan(&render_plan).unwrap()
    }

    fn fake_dds_dx10(width: u32, height: u32, dxgi_format: u32, payload_len: usize) -> Vec<u8> {
        let mut bytes = vec![0_u8; DDS_HEADER_BYTES + DDS_DX10_HEADER_BYTES + payload_len];
        bytes[..4].copy_from_slice(DDS_MAGIC);
        write_u32(&mut bytes, 4, 124);
        write_u32(&mut bytes, 12, height);
        write_u32(&mut bytes, 16, width);
        write_u32(&mut bytes, 80, DDPF_FOURCC);
        write_u32(&mut bytes, 84, DX10_FOURCC);
        write_u32(&mut bytes, DDS_HEADER_BYTES, dxgi_format);
        bytes
    }

    fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn unique_temp_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("borderless-magpie-{name}-{}", std::process::id()))
    }
}
