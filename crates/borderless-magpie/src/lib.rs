// SPDX-License-Identifier: GPL-3.0-or-later

#![forbid(unsafe_code)]

pub mod effect;
pub mod magpiefx;
pub mod profile;

pub use effect::{EffectGraph, EffectPass, EffectSource, MagpieEffect};
pub use magpiefx::{
    MagpieFx, MagpieFxParameter, MagpieFxPass, MagpieFxPassStyle, MagpieFxSampler,
    MagpieFxSamplerAddress, MagpieFxSamplerFilter, MagpieFxTexture, parse_magpiefx,
    parse_magpiefx_file,
};
pub use profile::{chaos_child_profile, tsukihime_profile};
