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

When porting a specific Magpie source file, check that file's own notice first. Use
`GPL-3.0-or-later` only for material that grants GPL version 3 or any later version. Shader assets and
small utility ports can have different licenses, so they must keep their own SPDX expression and
source attribution even when this crate's distribution boundary is GPL.

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
- `MagpieCompiledEffect`, which validates and bundles compiled shader bytecode with the renderer
  resource handoff.
- `MagpieExecutionPlan`, which lowers a compiled effect into ordered pass commands for pipeline,
  constant buffer, texture, sampler, and dispatch binding.
- `MagpieDispatchPlan`, which mirrors Magpie's `Dispatch(ceil(width / block_width), ceil(height /
  block_height), 1)` scheduling from the first output texture of each pass.
- `MagpieConstantBufferPlan`, which mirrors Magpie's CB1 dword layout for builtin constants,
  non-final PS-style pass size constants, and non-inline effect parameters.
- `MagpieResourcePlan`, which combines compile jobs, dispatch groups, CB1/CB2 bindings, SRV/UAV
  texture bindings, and sampler descriptors into the renderer handoff shape.
- `MagpieTextureAllocationPlan`, which resolves renderer texture allocations, usage flags, and
  read/write pass lifetimes.
- `MagpieTextureFormatDescriptor`, which maps Magpie texture formats to renderer-facing DXGI
  numbers, channel counts, component classes, and byte sizes when known.
- `MagpieSourceUploadPlan`, which prepares DDS `SOURCE` texture uploads with data offset, row
  pitch, byte size, and format metadata.
- `MagpieBackendDescriptorPlan`, which lowers resource plans into texture, buffer, and sampler
  descriptors ready for native DirectX or wgpu-specific translation.
- `MagpieWgpuDescriptorPlan` behind `wgpu-compare`, which converts backend descriptors into wgpu
  texture, buffer, and sampler descriptors for the comparison renderer path.
- `MagpieWgpuPreparedEffectPlan` behind `wgpu-compare`, which bundles the resource plan, backend
  descriptors, source uploads, binding layouts, WGSL declarations, translated shaders, and execution
  plan into one renderer handoff.
- `MagpieWgpuResourceObjects` behind `wgpu-compare`, which creates wgpu textures, texture views,
  constant buffers, and samplers from the backend descriptors and can assemble pass-local bind
  groups from the layout plan.
- `MagpieWgpuSourceUploadPlan` behind `wgpu-compare`, which validates `SOURCE` DDS payload ranges
  and writes them into wgpu textures through `Queue::write_texture`.
- `MagpieWgpuBindingLayoutPlan` behind `wgpu-compare`, which converts each pass's CBV/SRV/UAV
  and sampler resources into wgpu bind group layout entries. HLSL register classes are assigned
  disjoint wgpu binding bases so `b0`, `t0`, `u0`, and `s0` do not collide. The same plan can
  create pass-local `wgpu::BindGroupLayout` and `wgpu::PipelineLayout` objects once a renderer owns
  a `wgpu::Device`.
- `MagpieWgpuShaderPlan` behind `wgpu-compare`, which accepts WGSL pass sources produced by a
  future HLSL-to-WGSL translation layer and creates pass-local shader modules and compute pipelines
  against the explicit layout objects.
- `MagpieWgpuDeclarationPlan` behind `wgpu-compare`, which emits WGSL resource declarations from
  the Magpie resource plan using the same binding bases as the wgpu layout and bind group objects.
- `MagpieWgpuBodyTranslationPlan` behind `wgpu-compare`, which starts the HLSL-to-WGSL body
  translation path with simple compute-style texture copy passes.
- `MagpieWgpuExecutionPlan` behind `wgpu-compare`, which converts Magpie dispatch groups into
  ordered wgpu compute pass recording against pass-local pipelines and bind groups.

Bundled shader/effect assets are intentionally feature-gated and must carry per-file SPDX headers
from their original source. Some Magpie effect files have their own notices, so they should not be
blanket-relicensed from the Rust crate metadata.

Reference project:

- Magpie: <https://github.com/Blinue/Magpie>

Permissive crates such as `borderless-core` and `borderless-upscale-core` must not depend on this
crate unless the resulting build is intentionally GPL.
