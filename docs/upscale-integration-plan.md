<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# Upscale Integration Plan

The integrated scaler work is split into three lanes:

```mermaid
flowchart LR
    App["GUI / CLI"] --> Runtime["borderless-runtime"]
    Runtime --> Core["borderless-upscale-core\nMIT OR Apache-2.0"]
    Core --> Native["Native borderless\nMIT OR Apache-2.0"]
    Core --> GPL["GPL scaler boundary"]
    GPL --> Magpie["borderless-magpie\nMagpie-compatible port"]
    GPL --> Wgpu["borderless-upscale-wgpu\nDirectX/wgpu comparison"]
    Core --> Compat["retro compatibility boundary"]
    Compat --> Legacy["future borderless-compat\nDirectDraw / DirectInput / wrapper path"]
```

## Lanes

| Lane | Current crate | License | Purpose |
| --- | --- | --- | --- |
| Magpie style | `borderless-magpie` | GPL-3.0-or-later | Capture -> shader/effect graph -> fullscreen host. |
| IntegerScaler style | `borderless-upscale-core` | MIT OR Apache-2.0 | Source client size, integer scale, crop, and cursor clipping policy. |
| Retro compatibility | future `borderless-compat` | GPL or separate wrapper license | Legacy API wrapper, DirectInput remap, DirectDraw/GDI/Glide compatibility. |

## Current ADT boundary

`borderless-upscale-core` owns the clean Rust model:

```rust
enum CompatibilityMode {
    NativeBorderless,
    ProxyPresentation,
    LegacyWrapper,
    HookedPresentation,
}

enum CaptureBackend {
    GraphicsCapture,
    DesktopDuplication,
    DwmSharedSurface,
    Gdi,
}

enum ScalingBackend {
    Integer,
    Lanczos,
    Fsr1,
    Anime4k,
    MagpieFxCompatible,
    DirectMl,
}

enum InputBackend {
    NoRemap,
    ClipCursor,
    WindowMessageRemap,
    DirectInputShim,
    RawInputShim,
}

enum LegacyPresentationApi {
    None,
    Gdi,
    DirectDraw,
    Direct3d8,
    Direct3d9,
    Glide,
}

enum LegacyPresentationStrategy {
    None,
    BorderlessWindow,
    ProxyPresentation,
    WrappedPresentation,
}
```

The default build can use these types without linking GPL scaler crates. Enabling `magpie-port`
pulls in `borderless-magpie`; enabling `magpie-wgpu-compare` additionally pulls in the DirectX/wgpu
comparison crate.

## Retro compatibility layer

The retro-game path is intentionally separate from the Magpie upscale path. Magpie-compatible code
answers "how do we scale and present the final image"; the compatibility layer answers "how do we
make an old presentation or input model produce a reliable image and usable input first".

```mermaid
flowchart LR
    Game["legacy game"] --> Api["GDI / DirectDraw / D3D8 / D3D9 / Glide"]
    Api --> Compat["borderless-compat boundary"]
    Compat --> Frame["physical-pixel frame"]
    Compat --> Input["DirectInput / RawInput / cursor policy"]
    Frame --> Scale["integer / MagpieFX / future renderer"]
    Input --> Runtime["runtime restore and cleanup"]
```

The clean-room configuration lives in `borderless-upscale-core` as `LegacyCompatibilityProfile`.
That keeps profiles, GUI settings, and CLI flags permissively licensed while leaving any wrapper or
ported compatibility implementation in a dedicated crate. The planned implementation order is:

1. `BorderlessWindow`: use the existing native path for games that only need style/position fixes.
2. `ProxyPresentation`: capture the legacy client output and present it through the scaler host.
3. `WrappedPresentation`: interpose old APIs such as DirectDraw or DirectInput only when proxy
   capture cannot preserve timing, palette, cursor, or exclusive-mode assumptions.

This layer should prefer physical pixels at the Win32 boundary, preserve aspect ratio by default for
4:3/5:4 games, and keep taskbar/cursor restoration in the runtime cleanup path.

## Current implementation status

`borderless-magpie` now owns the GPL-only Magpie-compatible path. The first concrete slice is a
MagpieFX directive parser that keeps effect metadata separate from the shader body:

The parser, effect-asset compatibility checks, and render-resource planning in `borderless-magpie`
directly follow Magpie behavior and therefore remain GPL-3.0-or-later with Magpie attribution.

```mermaid
flowchart LR
    File["*.hlsl MagpieFX file"] --> Parser["parse_magpiefx"]
    Parser --> Metadata["VERSION / SORT_NAME / USE / CAPABILITY\nPARAMETER / TEXTURE / SAMPLER / COMMON / PASS"]
    Parser --> Shader["HLSL source without //! directives"]
    Metadata --> Graph["effect graph and renderer planning"]
    Metadata --> Resources["texture/resource plan"]
    Shader --> Package["MagpieEffectPackage\ninclude-expanded compiler source"]
    Resources --> Package
    Package --> Jobs["MagpieCompilePlan\n__M / cs_5_0 / per-pass macros"]
    Jobs --> Compiler["future DirectX shader compiler path"]
```

The parser currently recognizes these directive groups:

- global: `MAGPIE EFFECT`, `VERSION`, `SORT_NAME`, `USE`, `CAPABILITY`, `COMMON`;
- resources: `PARAMETER`, `LABEL`, `DEFAULT`, `MIN`, `MAX`, `STEP`, `TEXTURE`, `WIDTH`,
  `HEIGHT`, `FORMAT`, `SOURCE`, `SAMPLER`, `FILTER`;
- passes: `PASS`, `DESC`, `STYLE`, `IN`, `OUT`, `BLOCK_SIZE`, `NUM_THREADS`.

Validation requires `VERSION`, at least one texture, at least one pass, and explicit `INPUT` and
`OUTPUT` texture declarations. The native DirectX renderer, capture path, and shader compilation
are still separate follow-up slices.

The parser output can now be converted into `EffectGraph`, preserving pass style, inputs, outputs,
LUT/source texture references, compute block sizes, thread counts, and pass descriptions. That graph
is the handoff point for both the native DirectX renderer path and the later wgpu comparison path.

`MagpieRenderPlan` resolves MagpieFX textures into renderer-facing resources:

- `INPUT` and `OUTPUT` use the capture/output frame sizes;
- intermediate textures resolve simple Magpie dimension expressions such as `INPUT_WIDTH * 2`;
- `SOURCE` textures can be resolved from the effect file directory with `from_effect_file`;
- DDS source assets are read through a Pure Rust header parser for width, height, and DXGI format;
- pass inputs and outputs are validated against declared texture names.

The ignored `parse_magpie_assets` integration test can be pointed at a Magpie `src/Effects`
checkout with `MAGPIE_EFFECTS_DIR`; it parses, expands HLSL includes, resolves DDS source metadata,
and render-plans the effect assets as a compatibility gate.

`MagpieEffectPackage` is the current compiler handoff object. It contains the parsed `MagpieFx`, the
`MagpieRenderPlan`, the include-expanded HLSL compiler source, the resolved include list, and the
resolved `SOURCE` asset list.

`MagpieCompilePlan` derives per-pass shader jobs from a package. It follows Magpie's compute-shader
compile convention: every pass compiles through the `__M` entry point with `cs_5_0`, including
PS-style passes, and carries Magpie macros such as `MP_BLOCK_WIDTH`, `MP_NUM_THREADS_X`, `MP_PS_STYLE`,
`MP_FP16`, and the `MF*` float/min16float aliases.

## Renderer comparison rule

The Magpie-compatible native DirectX path is the reference. The wgpu path is eligible to replace it
only if benchmark summaries show comparable frame time and frame pacing under the same capture,
effect graph, and output rectangle.

The benchmark scaffold stores per-frame samples as:

```rust
struct BenchmarkSample {
    backend: RendererBackend,
    frame_index: u64,
    elapsed_nanos: u128,
}
```

Adoption criterion for the wgpu path:

- average frame time within 5% of the native DirectX path;
- p95 frame time within 10% of the native DirectX path;
- no additional frame drops in a fixed-duration capture replay.

## Implementation order

1. Keep `borderless-upscale-core` permissive and clean-room.
2. Port Magpie-compatible effect graph and renderer state into GPL crates.
3. Add capture backends one at a time: Graphics Capture first, then Desktop Duplication.
4. Add shader assets with per-file SPDX and source attribution.
5. Add wgpu/DirectX comparison path behind `magpie-wgpu-compare`.
6. Adopt wgpu only after benchmark parity is demonstrated.
