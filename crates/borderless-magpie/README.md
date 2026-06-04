<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# borderless-magpie

`borderless-magpie` contains the Magpie-compatible scaler port for Borderless Oxide.

This crate is licensed under GPL-3.0-or-later. Code in this crate that directly follows Magpie
behavior, file formats, render planning, shader semantics, or effect asset behavior inherits that GPL
boundary and carries Magpie attribution in the relevant source headers.

Rust files that directly port or closely follow Magpie behavior include `SPDX-FileCopyrightText`
entries for both Magpie and Borderless Oxide contributors. Local scaffolding in this crate remains
GPL because it links into the Magpie-compatible boundary, even when that individual file is not a
direct Magpie source port.

Current ported pieces include:

- MagpieFX directive parsing;
- conversion into `EffectGraph`;
- renderer-facing `MagpieRenderPlan`;
- Pure Rust DDS header metadata parsing for `SOURCE` textures;
- `MagpieEffectPackage`, which resolves includes, source assets, and render planning from an effect
  file;
- `MagpieCompilePlan`, which prepares Magpie-compatible per-pass shader compile jobs using the
  `__M` entry point, `cs_5_0` target profile, Magpie macro set, generated cbuffer/resource/sampler
  declarations, cbuffer or inline parameter bindings, optional `_DYNAMIC` / `MulAdd` helpers,
  built-in helpers, and per-pass wrapper source.
- `MagpieExternalHlslCompiler`, which can hand generated HLSL to `fxc` or `dxc` and return compiled
  bytecode plus diagnostics without adding unsafe Rust.
- `MagpieDispatchPlan`, which mirrors Magpie's `Dispatch(ceil(width / block_width), ceil(height /
  block_height), 1)` scheduling from the first output texture of each pass.
- `MagpieConstantBufferPlan`, which mirrors Magpie's CB1 dword layout for builtin constants,
  non-final PS-style pass size constants, and non-inline effect parameters.
- `MagpieResourcePlan`, which combines compile jobs, dispatch groups, CB1/CB2 bindings, SRV/UAV
  texture bindings, and sampler bindings into the renderer handoff shape.

Bundled shader/effect assets are intentionally feature-gated and must carry per-file SPDX headers
from their original source. Some Magpie effect files have their own notices, so they should not be
blanket-relicensed from the Rust crate metadata.

Reference project:

- Magpie: <https://github.com/Blinue/Magpie>

Permissive crates such as `borderless-core` and `borderless-upscale-core` must not depend on this
crate unless the resulting build is intentionally GPL.
