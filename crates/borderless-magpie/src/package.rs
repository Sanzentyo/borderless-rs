// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: Copyright (c) Xu and Magpie contributors
// SPDX-FileCopyrightText: Copyright (c) Borderless Oxide contributors
// Magpie-compatible effect packaging for the GPL port:
// https://github.com/Blinue/Magpie

use crate::magpiefx::{MagpieFx, parse_magpiefx_file};
use crate::plan::{MagpieRenderPlan, MagpieTextureRole};
use borderless_upscale_core::{FrameSize, UpscaleError, UpscaleResult};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieEffectPackage {
    pub effect_path: PathBuf,
    pub effect: MagpieFx,
    pub render_plan: MagpieRenderPlan,
    pub compiler_source: String,
    pub compiler_prelude_source: String,
    pub compiler_common_source: String,
    pub compiler_pass_sources: Vec<String>,
    pub includes: Vec<MagpieInclude>,
    pub source_assets: Vec<MagpieSourceAsset>,
}

impl MagpieEffectPackage {
    pub fn from_file(
        path: impl AsRef<Path>,
        input_size: FrameSize,
        output_size: FrameSize,
    ) -> UpscaleResult<Self> {
        let path = path.as_ref();
        let effect = parse_magpiefx_file(path)?;
        let source_dir = path.parent();
        let render_plan = MagpieRenderPlan::from_effect_with_source_dir(
            &effect,
            source_dir,
            input_size,
            output_size,
        )?;
        let source_root = path.parent().unwrap_or_else(|| Path::new("."));
        let mut resolver = IncludeResolver::new();
        let compiler_source = resolver.expand_source(&effect.hlsl_source, source_root)?;
        let compiler_prelude_source =
            IncludeResolver::new().expand_source(&effect.prelude_source, source_root)?;
        let compiler_common_source =
            IncludeResolver::new().expand_source(&effect.common_source, source_root)?;
        let compiler_pass_sources = effect
            .passes
            .iter()
            .map(|pass| IncludeResolver::new().expand_source(&pass.source, source_root))
            .collect::<UpscaleResult<Vec<_>>>()?;
        let source_assets = render_plan
            .textures
            .iter()
            .filter(|texture| texture.role == MagpieTextureRole::SourceAsset)
            .filter_map(|texture| {
                Some(MagpieSourceAsset {
                    texture_name: texture.name.clone(),
                    source: texture.source.clone()?,
                    path: texture.source_path.clone()?,
                    size: texture.size,
                    format: texture.format.clone(),
                })
            })
            .collect();

        Ok(Self {
            effect_path: path.to_path_buf(),
            effect,
            render_plan,
            compiler_source,
            compiler_prelude_source,
            compiler_common_source,
            compiler_pass_sources,
            includes: resolver.includes,
            source_assets,
        })
    }

    pub fn compiler_source_for_pass(&self, pass_index: usize) -> UpscaleResult<String> {
        let pass_source = self
            .compiler_pass_sources
            .get(pass_index.saturating_sub(1))
            .ok_or_else(|| {
                invalid_pipeline(format!("missing compiler source for pass {pass_index}"))
            })?;
        Ok(format!(
            "{}\n{}\n{}",
            self.compiler_prelude_source, self.compiler_common_source, pass_source
        ))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieInclude {
    pub requested: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieSourceAsset {
    pub texture_name: String,
    pub source: String,
    pub path: PathBuf,
    pub size: Option<FrameSize>,
    pub format: crate::plan::MagpieTextureFormat,
}

struct IncludeResolver {
    includes: Vec<MagpieInclude>,
    active: HashSet<PathBuf>,
}

impl IncludeResolver {
    fn new() -> Self {
        Self {
            includes: Vec::new(),
            active: HashSet::new(),
        }
    }

    fn expand_source(&mut self, source: &str, source_dir: &Path) -> UpscaleResult<String> {
        source.lines().try_fold(String::new(), |mut output, line| {
            if let Some(requested) = parse_include(line)? {
                output.push_str(&self.expand_include(&requested, source_dir)?);
            } else {
                output.push_str(line);
                output.push('\n');
            }
            Ok(output)
        })
    }

    fn expand_include(&mut self, requested: &str, source_dir: &Path) -> UpscaleResult<String> {
        let include_path = resolve_include_path(requested, source_dir)?;
        let canonical = include_path.canonicalize().map_err(|err| {
            UpscaleError::BackendUnavailable(format!(
                "failed to resolve include {}: {err}",
                include_path.display()
            ))
        })?;
        if !self.active.insert(canonical.clone()) {
            return Err(invalid_pipeline(format!(
                "recursive include detected: {}",
                include_path.display()
            )));
        }

        let include_source = std::fs::read_to_string(&canonical).map_err(|err| {
            UpscaleError::BackendUnavailable(format!(
                "failed to read include {}: {err}",
                canonical.display()
            ))
        })?;
        self.includes.push(MagpieInclude {
            requested: requested.to_owned(),
            path: canonical.clone(),
        });
        let include_dir = canonical.parent().unwrap_or(source_dir);
        let expanded = self.expand_source(&include_source, include_dir);
        self.active.remove(&canonical);
        expanded
    }
}

fn parse_include(line: &str) -> UpscaleResult<Option<String>> {
    let trimmed = line.trim();
    if !trimmed.starts_with("#include") {
        return Ok(None);
    }
    let value = trimmed.trim_start_matches("#include").trim();
    let requested = value
        .strip_prefix('"')
        .and_then(|value| value.split_once('"').map(|(include, _)| include))
        .or_else(|| {
            value
                .strip_prefix('<')
                .and_then(|value| value.split_once('>').map(|(include, _)| include))
        })
        .ok_or_else(|| invalid_pipeline(format!("invalid include directive: {line}")))?;
    Ok(Some(requested.to_owned()))
}

fn resolve_include_path(requested: &str, source_dir: &Path) -> UpscaleResult<PathBuf> {
    let requested = Path::new(requested);
    if requested.is_absolute() {
        return Err(invalid_pipeline(format!(
            "absolute includes are not supported: {}",
            requested.display()
        )));
    }
    Ok(source_dir.join(requested))
}

fn invalid_pipeline(message: impl Into<String>) -> UpscaleError {
    UpscaleError::InvalidPipeline(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_relative_includes_and_collects_source_assets() {
        let root = std::env::temp_dir().join(format!(
            "borderless-magpie-package-test-{}",
            std::process::id()
        ));
        let nested = root.join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(root.join("common.hlsli"), "float Common() { return 1; }\n").unwrap();
        std::fs::write(nested.join("AreaTex.dds"), fake_dds_dx10(160, 560, 61)).unwrap();
        let effect_path = nested.join("Effect.hlsl");
        std::fs::write(
            &effect_path,
            r#"
//!MAGPIE EFFECT
//!VERSION 4
#include "../common.hlsli"
//!TEXTURE
Texture2D INPUT;
//!TEXTURE
Texture2D OUTPUT;
//!TEXTURE
//!SOURCE AreaTex.dds
Texture2D areaTex;
//!PASS 1
//!IN INPUT, areaTex
//!OUT OUTPUT
float4 Pass1() { return Common(); }
"#,
        )
        .unwrap();

        let package = MagpieEffectPackage::from_file(
            &effect_path,
            FrameSize::new(640, 480).unwrap(),
            FrameSize::new(1280, 960).unwrap(),
        )
        .unwrap();

        assert!(package.compiler_source.contains("float Common()"));
        assert!(package.compiler_prelude_source.contains("float Common()"));
        assert!(
            package
                .compiler_source_for_pass(1)
                .unwrap()
                .contains("Pass1")
        );
        assert!(!package.compiler_source.contains("#include"));
        assert_eq!(package.includes.len(), 1);
        assert_eq!(package.source_assets.len(), 1);
        assert_eq!(package.source_assets[0].size, FrameSize::new(160, 560));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_recursive_includes() {
        let root = std::env::temp_dir().join(format!(
            "borderless-magpie-recursive-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("a.hlsli"), "#include \"b.hlsli\"\n").unwrap();
        std::fs::write(root.join("b.hlsli"), "#include \"a.hlsli\"\n").unwrap();

        let mut resolver = IncludeResolver::new();
        let result = resolver.expand_source("#include \"a.hlsli\"\n", &root);

        let _ = std::fs::remove_dir_all(root);
        assert!(matches!(result, Err(UpscaleError::InvalidPipeline(_))));
    }

    fn fake_dds_dx10(width: u32, height: u32, dxgi_format: u32) -> Vec<u8> {
        const DDS_MAGIC: &[u8; 4] = b"DDS ";
        const MIN_DDS_SIZE: usize = 128;
        const DDPF_FOURCC: u32 = 0x0000_0004;
        const DX10_FOURCC: u32 = u32::from_le_bytes(*b"DX10");

        let mut bytes = vec![0_u8; MIN_DDS_SIZE + 20];
        bytes[..4].copy_from_slice(DDS_MAGIC);
        write_u32(&mut bytes, 4, 124);
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
