<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# SPDX and GPL Boundary

`borderless-rs` keeps the reusable domain/runtime crates permissively licensed where possible, while
placing Magpie-derived scaler work under GPL.

## Crate license boundary

| Crate | SPDX license expression | Notes |
| --- | --- | --- |
| `borderless-core` | `MIT OR Apache-2.0` | Pure domain types, traits, and planning logic. |
| `borderless-native` | `MIT OR Apache-2.0` | Win32 manipulation and capture-facing boundaries that do not include Magpie-derived code. |
| `borderless-runtime` | `MIT OR Apache-2.0` | Actor/controller orchestration. |
| `borderless-cli` | `MIT OR Apache-2.0` by default | Enabling `magpie-port` links GPL crates and makes that build GPL. |
| `borderless-gui` | `MIT OR Apache-2.0` by default | Enabling `magpie-port` links GPL crates and makes that build GPL. |
| `borderless-upscale-core` | `MIT OR Apache-2.0` | Clean Rust ADTs and traits for capture/scaling/input/presentation. |
| `borderless-magpie` | `GPL-3.0-or-later` | Magpie-compatible port layer and effect graph. |
| `borderless-upscale-wgpu` | `GPL-3.0-or-later` | wgpu/DirectX rewrite derived from the Magpie-compatible pipeline. |

## Magpie attribution and inheritance

The Magpie-compatible implementation references the GPL-licensed Magpie project:

- Magpie: <https://github.com/Blinue/Magpie>

Any Rust module, shader, test fixture, render plan, parser behavior, or asset handling that directly
follows Magpie behavior is kept in GPL-3.0-or-later crates. This includes `borderless-magpie` and
GPL comparison/rendering crates that consume Magpie-derived pipeline behavior.

Clean-room planning types may remain in permissive crates only when they are not derived from Magpie
implementation details and can stand independently as generic capture/scaling/input abstractions.

Magpie source files commonly carry a GPL v3-or-later header, while the repository-level README
summarizes the project as GPLv3. For Rust code that ports Magpie implementation behavior directly,
use `GPL-3.0-or-later` only when the referenced Magpie source grants the "or later" option. Bundled
effect or shader assets must keep the license expression and attribution from the specific source
file; do not assume that every effect asset can use `GPL-3.0-or-later`.

## Current Magpie-derived files

These files directly follow Magpie behavior and therefore carry Magpie attribution in their SPDX
headers:

| File | Magpie-derived behavior |
| --- | --- |
| `crates/borderless-magpie/src/magpiefx.rs` | MagpieFX directive parsing and shader block splitting. |
| `crates/borderless-magpie/src/plan.rs` | Magpie texture planning, pass input/output rules, and effect size expression handling. |
| `crates/borderless-magpie/src/dds.rs` | DDS metadata handling for Magpie `SOURCE` textures. |
| `crates/borderless-magpie/src/package.rs` | Magpie effect packaging, include expansion, and compiler handoff layout. |
| `crates/borderless-magpie/src/compiler.rs` | Magpie-style per-pass compile jobs, macros, entry point, and wrapper generation. |
| `crates/borderless-magpie/tests/parse_magpie_assets.rs` | Compatibility tests against Magpie's GPL effect assets. |

These files are GPL because they live inside the Magpie-compatible crate boundary, but they are
currently Borderless Oxide implementation scaffolding rather than direct Magpie source ports:

| File | Reason |
| --- | --- |
| `crates/borderless-magpie/src/effect.rs` | Generic Rust ADTs for the local effect graph. |
| `crates/borderless-magpie/src/profile.rs` | Local prototype profiles for target games and compatibility experiments. |
| `crates/borderless-magpie/src/lib.rs` | Crate module/export boundary. |

## Feature boundary

- `magpie-port`: enables GPL Magpie-compatible scaler integration.
- `magpie-wgpu-compare`: enables the GPL wgpu/DirectX comparison path.
- `shader-assets`: reserved for bundled shader ports. Shader files must carry their own SPDX header
  and source attribution because shader licenses can differ from the Rust crate license.

The default build does not enable GPL scaler crates. Any binary distributed with `magpie-port`,
`magpie-wgpu-compare`, or bundled GPL shader assets should be treated as GPL-3.0-or-later.

## Source header policy

New source files should include one SPDX header:

```text
// SPDX-License-Identifier: MIT OR Apache-2.0
```

or:

```text
// SPDX-License-Identifier: GPL-3.0-or-later
```

Files that directly port or closely follow Magpie should also include:

```text
// SPDX-FileCopyrightText: Copyright (c) Xu and Magpie contributors
// SPDX-FileCopyrightText: Copyright (c) Borderless Oxide contributors
```

If a future Magpie-derived file is based on a source file that lacks an "or later" grant, use the
stricter SPDX expression from that source file instead.

Markdown documentation can use an HTML comment:

```text
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
```
