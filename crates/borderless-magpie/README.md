<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# borderless-magpie

`borderless-magpie` contains the Magpie-compatible scaler port for Borderless Oxide.

This crate is licensed under GPL-3.0-or-later. Code in this crate that directly follows Magpie
behavior, file formats, render planning, shader semantics, or effect asset behavior inherits that GPL
boundary.

Current ported pieces include:

- MagpieFX directive parsing;
- conversion into `EffectGraph`;
- renderer-facing `MagpieRenderPlan`;
- Pure Rust DDS header metadata parsing for `SOURCE` textures;
- `MagpieEffectPackage`, which resolves includes, source assets, and render planning from an effect
  file.

Reference project:

- Magpie: <https://github.com/Blinue/Magpie>

Permissive crates such as `borderless-core` and `borderless-upscale-core` must not depend on this
crate unless the resulting build is intentionally GPL.
