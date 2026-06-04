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
follows Magpie behavior must inherit the license of the Magpie source material it follows. In
practice, the current Rust Magpie pipeline ports code and behavior from Magpie source files that grant
GPL version 3 or any later version, so those files are kept in GPL-3.0-or-later crates. This includes
`borderless-magpie` and GPL comparison/rendering crates that consume Magpie-derived pipeline
behavior.

Clean-room planning types may remain in permissive crates only when they are not derived from Magpie
implementation details and can stand independently as generic capture/scaling/input abstractions.

Magpie source files commonly carry a GPL v3-or-later header, while the repository-level README
summarizes the project as GPLv3. For Rust code that ports Magpie implementation behavior directly,
use `GPL-3.0-or-later` only when the referenced Magpie source grants the "or later" option. If the
source only says GPLv3, use `GPL-3.0-only`. If it says GPLv2 or later, MIT, Apache, or another
license, keep that exact source-file expression unless the file is intentionally absorbed into the
GPL crate boundary. Bundled effect or shader assets must keep the license expression and attribution
from the specific source file; do not assume that every effect asset can use `GPL-3.0-or-later`.

## Current Magpie-derived files

These files directly follow Magpie behavior and therefore carry Magpie attribution in their SPDX
headers. The current Rust source expression is `GPL-3.0-or-later` because these ports follow Magpie
application/pipeline source that grants GPLv3-or-later, or because the file is intentionally inside
the GPL Magpie-compatible crate boundary.

| File | Magpie-derived behavior |
| --- | --- |
| `crates/borderless-magpie/src/allocation.rs` | Renderer texture allocation, usage flags, and pass lifetime planning. |
| `crates/borderless-magpie/src/backend.rs` | Renderer-facing texture, buffer, and sampler descriptor lowering. |
| `crates/borderless-magpie/src/compiled.rs` | Compiled effect bundling and renderer handoff shape. |
| `crates/borderless-magpie/src/compiler.rs` | Magpie-style per-pass compile jobs, macros, entry point, and wrapper generation. |
| `crates/borderless-magpie/src/constants.rs` | Magpie CB1 builtin constant and parameter layout planning. |
| `crates/borderless-magpie/src/dds.rs` | DDS metadata handling for Magpie `SOURCE` textures. Magpie's `DDS.h` is MIT, but this file is kept GPL because it lives in the Magpie-compatible crate boundary. |
| `crates/borderless-magpie/src/dispatch.rs` | Magpie-style compute dispatch group planning from output texture size and block size. |
| `crates/borderless-magpie/src/execution.rs` | Ordered renderer execution commands for compiled Magpie-compatible passes. |
| `crates/borderless-magpie/src/formats.rs` | Magpie texture format metadata and DXGI mapping. |
| `crates/borderless-magpie/src/hlsl_compiler.rs` | External HLSL compiler handoff for generated Magpie-compatible shader jobs. |
| `crates/borderless-magpie/src/magpiefx.rs` | MagpieFX directive parsing and shader block splitting. |
| `crates/borderless-magpie/src/package.rs` | Magpie effect packaging, include expansion, and compiler handoff layout. |
| `crates/borderless-magpie/src/plan.rs` | Magpie texture planning, pass input/output rules, and effect size expression handling. |
| `crates/borderless-magpie/src/resources.rs` | Combined compile, dispatch, constant buffer, texture, and sampler resource planning. |
| `crates/borderless-magpie/src/upload.rs` | Magpie `SOURCE` texture upload planning. |
| `crates/borderless-magpie/src/wgpu_bridge.rs` | wgpu comparison bridge derived from the Magpie-compatible resource/pipeline behavior. |
| `crates/borderless-magpie/tests/parse_magpie_assets.rs` | Compatibility tests against Magpie effect assets. |

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

When a future implementation directly ports from a permissively licensed Magpie utility file, prefer
keeping that implementation outside the GPL Magpie crate if it can remain independent. If the file
must live in `borderless-magpie`, document both facts in this file: the original permissive source
license and the crate-level GPL boundary that applies to distribution of this crate.

Markdown documentation can use an HTML comment:

```text
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
```
