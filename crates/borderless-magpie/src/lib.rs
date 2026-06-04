// SPDX-License-Identifier: GPL-3.0-or-later

#![forbid(unsafe_code)]

pub mod allocation;
pub mod backend;
pub mod compiled;
pub mod compiler;
pub mod constants;
pub mod dds;
pub mod dispatch;
pub mod effect;
pub mod execution;
pub mod formats;
pub mod hlsl_compiler;
pub mod magpiefx;
pub mod package;
pub mod plan;
pub mod profile;
pub mod resources;
pub mod upload;

pub use allocation::{MagpieTextureAllocation, MagpieTextureAllocationPlan, MagpieTextureUsage};
pub use backend::{
    MagpieBackendBufferDescriptor, MagpieBackendDescriptorPlan, MagpieBackendSamplerDescriptor,
    MagpieBackendTextureBind, MagpieBackendTextureDescriptor,
};
pub use compiled::{MagpieCompiledEffect, MagpieCompiledEffectOptions};
pub use compiler::{
    MAGPIE_ENTRY_POINT, MAGPIE_TARGET_PROFILE, MagpieCompilePlan, MagpieShaderJob,
    MagpieShaderMacro,
};
pub use constants::{
    MagpieConstantBufferOptions, MagpieConstantBufferPlan, MagpieConstantEntry,
    MagpieConstantValue, MagpieDynamicConstantBuffer, MagpieParameterValue,
};
pub use dds::{DdsMetadata, parse_dds_metadata, read_dds_metadata};
pub use dispatch::{MagpieDispatchPass, MagpieDispatchPlan};
pub use effect::{EffectGraph, EffectPass, EffectPassStyle, EffectSource, MagpieEffect};
pub use execution::{MagpieExecutionCommand, MagpieExecutionPlan};
pub use formats::{MagpieTextureComponent, MagpieTextureFormatDescriptor};
pub use hlsl_compiler::{
    MagpieCompiledShader, MagpieExternalHlslCompiler, MagpieHlslCompilerInvocation,
    MagpieHlslCompilerKind,
};
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
pub use resources::{
    MAGPIE_CB1_REGISTER, MAGPIE_CB2_REGISTER, MagpieConstantBufferBinding, MagpieResourcePass,
    MagpieResourcePlan, MagpieResourcePlanOptions, MagpieSamplerResourceBinding,
    MagpieTextureResourceBinding,
};
pub use upload::{MagpieSourceUpload, MagpieSourceUploadPlan};
