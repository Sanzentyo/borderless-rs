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

Markdown documentation can use an HTML comment:

```text
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
```
