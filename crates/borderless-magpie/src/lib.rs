// SPDX-License-Identifier: GPL-3.0-or-later

#![forbid(unsafe_code)]

pub mod effect;
pub mod profile;

pub use effect::{EffectGraph, EffectPass, EffectSource, MagpieEffect};
pub use profile::{chaos_child_profile, tsukihime_profile};
