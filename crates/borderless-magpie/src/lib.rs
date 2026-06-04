// SPDX-License-Identifier: GPL-3.0-or-later

#![forbid(unsafe_code)]

pub mod compiler;
pub mod dds;
pub mod effect;
pub mod magpiefx;
pub mod package;
pub mod plan;
pub mod profile;

pub use compiler::{
    MAGPIE_ENTRY_POINT, MAGPIE_TARGET_PROFILE, MagpieCompilePlan, MagpieShaderJob,
    MagpieShaderMacro,
};
pub use dds::{DdsMetadata, parse_dds_metadata, read_dds_metadata};
pub use effect::{EffectGraph, EffectPass, EffectPassStyle, EffectSource, MagpieEffect};
pub use magpiefx::{
    MagpieFx, MagpieFxParameter, MagpieFxPass, MagpieFxPassStyle, MagpieFxSampler,
    MagpieFxSamplerAddress, MagpieFxSamplerFilter, MagpieFxTexture, parse_magpiefx,
    parse_magpiefx_file,
};
pub use package::{MagpieEffectPackage, MagpieInclude, MagpieSourceAsset};
pub use plan::{
    MagpiePassPlan, MagpieRenderPlan, MagpieTextureFormat, MagpieTexturePlan, MagpieTextureRole,
};
pub use profile::{chaos_child_profile, tsukihime_profile};
