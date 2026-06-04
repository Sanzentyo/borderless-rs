// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: Copyright (c) Xu and Magpie contributors
// SPDX-FileCopyrightText: Copyright (c) Borderless Oxide contributors
// Magpie-compatible shader compilation planning for the GPL port:
// https://github.com/Blinue/Magpie

use crate::magpiefx::{MagpieFxParameter, MagpieFxParameterType, MagpieFxPassStyle};
use crate::package::MagpieEffectPackage;
use crate::plan::{MagpiePassPlan, MagpieTextureFormat, MagpieTexturePlan};
use borderless_upscale_core::{UpscaleError, UpscaleResult};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

pub const MAGPIE_ENTRY_POINT: &str = "__M";
pub const MAGPIE_TARGET_PROFILE: &str = "cs_5_0";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieCompilePlan {
    pub source_name: String,
    pub jobs: Vec<MagpieShaderJob>,
}

impl MagpieCompilePlan {
    pub fn from_package(package: &MagpieEffectPackage) -> UpscaleResult<Self> {
        let source_name = package
            .effect_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("effect.hlsl")
            .to_owned();
        let jobs = package
            .render_plan
            .passes
            .iter()
            .enumerate()
            .map(|(index, pass)| {
                let base_source = package.compiler_source_for_pass(index + 1)?;
                shader_job(index + 1, pass, &base_source, package)
            })
            .collect::<UpscaleResult<Vec<_>>>()?;

        Ok(Self { source_name, jobs })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieShaderJob {
    pub pass_index: u32,
    pub pass_name: String,
    pub pass_function: String,
    pub entry_point: String,
    pub target_profile: String,
    pub style: MagpieFxPassStyle,
    pub block_size: [u32; 2],
    pub num_threads: [u32; 3],
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub texture_bindings: Vec<MagpieTextureBinding>,
    pub sampler_bindings: Vec<MagpieSamplerBinding>,
    pub parameter_bindings: Vec<MagpieParameterBinding>,
    pub base_source: String,
    pub generated_source: String,
    pub macros: Vec<MagpieShaderMacro>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieTextureBinding {
    pub name: String,
    pub register: u32,
    pub format: MagpieTextureFormat,
    pub texel_type: String,
    pub access: MagpieTextureAccess,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MagpieTextureAccess {
    ShaderResource,
    UnorderedAccess,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieSamplerBinding {
    pub name: String,
    pub register: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieParameterBinding {
    pub name: String,
    pub value_type: MagpieFxParameterType,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieShaderMacro {
    pub name: String,
    pub value: Option<String>,
}

impl MagpieShaderMacro {
    #[must_use]
    pub fn define(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: None,
        }
    }

    #[must_use]
    pub fn value(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: Some(value.into()),
        }
    }
}

fn shader_job(
    pass_index: usize,
    pass: &MagpiePassPlan,
    base_source: &str,
    package: &MagpieEffectPackage,
) -> UpscaleResult<MagpieShaderJob> {
    let block_size = normalized_block_size(pass)?;
    let num_threads = normalized_num_threads(pass)?;
    let pass_index = u32::try_from(pass_index).map_err(|_| invalid_pipeline("too many passes"))?;
    let texture_bindings = texture_bindings(pass, &package.render_plan.textures)?;
    let sampler_bindings = sampler_bindings(package)?;
    let parameter_bindings = parameter_bindings(package)?;
    let use_flags = MagpieUseFlags::from_uses(&package.effect.uses);
    let mut macros = vec![
        MagpieShaderMacro::value("MP_BLOCK_WIDTH", block_size[0].to_string()),
        MagpieShaderMacro::value("MP_BLOCK_HEIGHT", block_size[1].to_string()),
        MagpieShaderMacro::value("MP_NUM_THREADS_X", num_threads[0].to_string()),
        MagpieShaderMacro::value("MP_NUM_THREADS_Y", num_threads[1].to_string()),
        MagpieShaderMacro::value("MP_NUM_THREADS_Z", num_threads[2].to_string()),
    ];
    if pass.style == MagpieFxPassStyle::PixelShader {
        macros.push(MagpieShaderMacro::define("MP_PS_STYLE"));
    }
    extend_float_macros(
        &mut macros,
        package
            .effect
            .capabilities
            .iter()
            .any(|value| value == "FP16"),
    );
    let generated_source = generate_pass_source(
        pass_index,
        pass,
        block_size,
        num_threads,
        PassSourceContext {
            base_source,
            texture_bindings: &texture_bindings,
            sampler_bindings: &sampler_bindings,
            parameter_bindings: &parameter_bindings,
            all_passes: &package.render_plan.passes,
            use_flags,
        },
    )?;

    Ok(MagpieShaderJob {
        pass_index,
        pass_name: pass
            .description
            .clone()
            .unwrap_or_else(|| format!("Pass {pass_index}")),
        pass_function: format!("Pass{pass_index}"),
        entry_point: MAGPIE_ENTRY_POINT.to_owned(),
        target_profile: MAGPIE_TARGET_PROFILE.to_owned(),
        style: pass.style,
        block_size,
        num_threads,
        inputs: pass.inputs.clone(),
        outputs: pass.outputs.clone(),
        texture_bindings,
        sampler_bindings,
        parameter_bindings,
        base_source: base_source.to_owned(),
        generated_source,
        macros,
    })
}

fn generate_pass_source(
    pass_index: u32,
    pass: &MagpiePassPlan,
    block_size: [u32; 2],
    num_threads: [u32; 3],
    context: PassSourceContext<'_>,
) -> UpscaleResult<String> {
    validate_pass_function(context.base_source, pass_index)?;
    let wrapper = match pass.style {
        MagpieFxPassStyle::PixelShader => {
            generate_pixel_style_entry(pass_index, pass, context.all_passes.len())?
        }
        MagpieFxPassStyle::Compute => {
            generate_compute_style_entry(pass_index, block_size, num_threads)
        }
    };
    let prelude = generate_shader_prelude(
        context.texture_bindings,
        context.sampler_bindings,
        context.parameter_bindings,
        context.all_passes,
        context.use_flags,
    );
    Ok(format!("{prelude}{}\n\n{wrapper}", context.base_source))
}

#[derive(Clone, Copy)]
struct PassSourceContext<'a> {
    base_source: &'a str,
    texture_bindings: &'a [MagpieTextureBinding],
    sampler_bindings: &'a [MagpieSamplerBinding],
    parameter_bindings: &'a [MagpieParameterBinding],
    all_passes: &'a [MagpiePassPlan],
    use_flags: MagpieUseFlags,
}

fn validate_pass_function(source: &str, pass_index: u32) -> UpscaleResult<()> {
    let function = format!("Pass{pass_index}");
    source
        .contains(&function)
        .then_some(())
        .ok_or_else(|| invalid_pipeline(format!("Magpie shader source is missing {function}")))
}

fn texture_bindings(
    pass: &MagpiePassPlan,
    textures: &[MagpieTexturePlan],
) -> UpscaleResult<Vec<MagpieTextureBinding>> {
    let inputs = pass.inputs.iter().enumerate().map(|(index, name)| {
        let texture = texture_by_name(textures, name)?;
        Ok(texture_binding(
            texture,
            index,
            MagpieTextureAccess::ShaderResource,
        ))
    });
    let outputs = pass.outputs.iter().enumerate().map(|(index, name)| {
        let texture = texture_by_name(textures, name)?;
        Ok(texture_binding(
            texture,
            index,
            MagpieTextureAccess::UnorderedAccess,
        ))
    });
    inputs.chain(outputs).collect()
}

fn texture_by_name<'a>(
    textures: &'a [MagpieTexturePlan],
    name: &str,
) -> UpscaleResult<&'a MagpieTexturePlan> {
    textures
        .iter()
        .find(|texture| texture.name == name)
        .ok_or_else(|| invalid_pipeline(format!("pass references unknown texture {name}")))
}

fn texture_binding(
    texture: &MagpieTexturePlan,
    register: usize,
    access: MagpieTextureAccess,
) -> MagpieTextureBinding {
    let texel_type = match access {
        MagpieTextureAccess::ShaderResource => hlsl_srv_texel_type(&texture.format),
        MagpieTextureAccess::UnorderedAccess => hlsl_uav_texel_type(&texture.format),
    };
    MagpieTextureBinding {
        name: texture.name.clone(),
        register: u32::try_from(register).expect("pass IO is already bounded"),
        format: texture.format.clone(),
        texel_type: texel_type.to_owned(),
        access,
    }
}

fn sampler_bindings(package: &MagpieEffectPackage) -> UpscaleResult<Vec<MagpieSamplerBinding>> {
    package
        .effect
        .samplers
        .iter()
        .enumerate()
        .map(|(index, sampler)| {
            Ok(MagpieSamplerBinding {
                name: sampler.name.clone(),
                register: u32::try_from(index)
                    .map_err(|_| invalid_pipeline("too many Magpie samplers"))?,
            })
        })
        .collect()
}

fn parameter_bindings(package: &MagpieEffectPackage) -> UpscaleResult<Vec<MagpieParameterBinding>> {
    package
        .effect
        .parameters
        .iter()
        .map(parameter_binding)
        .collect()
}

fn parameter_binding(parameter: &MagpieFxParameter) -> UpscaleResult<MagpieParameterBinding> {
    if parameter.value_type == MagpieFxParameterType::Unknown {
        return Err(invalid_pipeline(format!(
            "Magpie parameter {} has unsupported type",
            parameter.symbol
        )));
    }
    Ok(MagpieParameterBinding {
        name: parameter.symbol.clone(),
        value_type: parameter.value_type,
    })
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct MagpieUseFlags {
    dynamic: bool,
    mul_add: bool,
}

impl MagpieUseFlags {
    fn from_uses(uses: &[String]) -> Self {
        uses.iter().fold(Self::default(), |mut flags, value| {
            match value.to_ascii_uppercase().as_str() {
                "MULADD" => flags.mul_add = true,
                "_DYNAMIC" | "DYNAMIC" => flags.dynamic = true,
                _ => {}
            }
            flags
        })
    }
}

fn generate_shader_prelude(
    texture_bindings: &[MagpieTextureBinding],
    sampler_bindings: &[MagpieSamplerBinding],
    parameter_bindings: &[MagpieParameterBinding],
    all_passes: &[MagpiePassPlan],
    use_flags: MagpieUseFlags,
) -> String {
    let mut source = String::new();
    source.push_str(&constant_buffer_source(all_passes, parameter_bindings));
    if use_flags.dynamic {
        source.push_str("cbuffer __CB2 : register(b1) { uint __frameCount; };\n\n");
    }
    append_resource_declarations(&mut source, texture_bindings, sampler_bindings);
    source.push_str(BUILTIN_FUNCTIONS);
    if use_flags.mul_add {
        source.push_str(MUL_ADD_FUNCTIONS);
    }
    source.push('\n');
    source
}

fn constant_buffer_source(
    all_passes: &[MagpiePassPlan],
    parameter_bindings: &[MagpieParameterBinding],
) -> String {
    let mut source = String::from(
        "cbuffer __CB1 : register(b0) {\n\
    uint2 __inputSize;\n\
    uint2 __outputSize;\n\
    float2 __inputPt;\n\
    float2 __outputPt;\n\
    float2 __scale;\n",
    );
    for (index, pass) in all_passes
        .iter()
        .enumerate()
        .take(all_passes.len().saturating_sub(1))
    {
        if pass.style == MagpieFxPassStyle::PixelShader {
            let pass_index = index + 1;
            writeln!(
                &mut source,
                "    uint2 __pass{pass_index}OutputSize;\n    float2 __pass{pass_index}OutputPt;"
            )
            .expect("writing to String cannot fail");
        }
    }
    for parameter in parameter_bindings {
        writeln!(
            &mut source,
            "    {} {};",
            hlsl_parameter_type(parameter.value_type),
            parameter.name
        )
        .expect("writing to String cannot fail");
    }
    source.push_str("};\n\n");
    source
}

fn hlsl_parameter_type(value_type: MagpieFxParameterType) -> &'static str {
    match value_type {
        MagpieFxParameterType::Float | MagpieFxParameterType::Unknown => "float",
        MagpieFxParameterType::Int => "int",
    }
}

fn append_resource_declarations(
    source: &mut String,
    texture_bindings: &[MagpieTextureBinding],
    sampler_bindings: &[MagpieSamplerBinding],
) {
    for binding in texture_bindings
        .iter()
        .filter(|binding| binding.access == MagpieTextureAccess::ShaderResource)
    {
        writeln!(
            source,
            "Texture2D<{}> {} : register(t{});",
            binding.texel_type, binding.name, binding.register
        )
        .expect("writing to String cannot fail");
    }
    for binding in texture_bindings
        .iter()
        .filter(|binding| binding.access == MagpieTextureAccess::UnorderedAccess)
    {
        writeln!(
            source,
            "RWTexture2D<{}> {} : register(u{});",
            binding.texel_type, binding.name, binding.register
        )
        .expect("writing to String cannot fail");
    }
    for binding in sampler_bindings {
        writeln!(
            source,
            "SamplerState {} : register(s{});",
            binding.name, binding.register
        )
        .expect("writing to String cannot fail");
    }
    source.push('\n');
}

fn hlsl_srv_texel_type(format: &MagpieTextureFormat) -> &'static str {
    match format {
        MagpieTextureFormat::R16g16b16a16Float
        | MagpieTextureFormat::R8g8b8a8Unorm
        | MagpieTextureFormat::R8g8b8a8UnormSrgb
        | MagpieTextureFormat::R8g8b8a8Snorm => "MF4",
        MagpieTextureFormat::R16g16Float | MagpieTextureFormat::R8g8Unorm => "MF2",
        MagpieTextureFormat::R32Float => "float",
        MagpieTextureFormat::R16Float | MagpieTextureFormat::R8Unorm => "MF",
        MagpieTextureFormat::R32g32b32a32Float
        | MagpieTextureFormat::Dxgi(_)
        | MagpieTextureFormat::Unknown(_) => "float4",
    }
}

fn hlsl_uav_texel_type(format: &MagpieTextureFormat) -> &'static str {
    match format {
        MagpieTextureFormat::R16g16b16a16Float => "MF4",
        MagpieTextureFormat::R8g8b8a8Unorm | MagpieTextureFormat::R8g8b8a8UnormSrgb => "unorm MF4",
        MagpieTextureFormat::R8g8b8a8Snorm => "snorm MF4",
        MagpieTextureFormat::R16g16Float => "MF2",
        MagpieTextureFormat::R8g8Unorm => "unorm MF2",
        MagpieTextureFormat::R32Float => "float",
        MagpieTextureFormat::R16Float => "MF",
        MagpieTextureFormat::R8Unorm => "unorm MF",
        MagpieTextureFormat::R32g32b32a32Float
        | MagpieTextureFormat::Dxgi(_)
        | MagpieTextureFormat::Unknown(_) => "float4",
    }
}

const BUILTIN_FUNCTIONS: &str = r"uint __Bfe(uint src, uint off, uint bits) { uint mask = (1u << bits) - 1; return (src >> off) & mask; }
uint __BfiM(uint src, uint ins, uint bits) { uint mask = (1u << bits) - 1; return (ins & mask) | (src & (~mask)); }
uint2 Rmp8x8(uint a) { return uint2(__Bfe(a, 1u, 3u), __BfiM(__Bfe(a, 3u, 3u), a, 1u)); }
uint2 GetInputSize() { return __inputSize; }
float2 GetInputPt() { return __inputPt; }
uint2 GetOutputSize() { return __outputSize; }
float2 GetOutputPt() { return __outputPt; }
float2 GetScale() { return __scale; }
";

const MUL_ADD_FUNCTIONS: &str = r"MF2 MulAdd(MF2 x, MF2x2 y, MF2 a) {
    MF2 result = a;
    result = mad(x.x, y._m00_m01, result);
    result = mad(x.y, y._m10_m11, result);
    return result;
}
MF3 MulAdd(MF2 x, MF2x3 y, MF3 a) {
    MF3 result = a;
    result = mad(x.x, y._m00_m01_m02, result);
    result = mad(x.y, y._m10_m11_m12, result);
    return result;
}
MF4 MulAdd(MF2 x, MF2x4 y, MF4 a) {
    MF4 result = a;
    result = mad(x.x, y._m00_m01_m02_m03, result);
    result = mad(x.y, y._m10_m11_m12_m13, result);
    return result;
}
MF2 MulAdd(MF3 x, MF3x2 y, MF2 a) {
    MF2 result = a;
    result = mad(x.x, y._m00_m01, result);
    result = mad(x.y, y._m10_m11, result);
    result = mad(x.z, y._m20_m21, result);
    return result;
}
MF3 MulAdd(MF3 x, MF3x3 y, MF3 a) {
    MF3 result = a;
    result = mad(x.x, y._m00_m01_m02, result);
    result = mad(x.y, y._m10_m11_m12, result);
    result = mad(x.z, y._m20_m21_m22, result);
    return result;
}
MF4 MulAdd(MF3 x, MF3x4 y, MF4 a) {
    MF4 result = a;
    result = mad(x.x, y._m00_m01_m02_m03, result);
    result = mad(x.y, y._m10_m11_m12_m13, result);
    result = mad(x.z, y._m20_m21_m22_m23, result);
    return result;
}
MF2 MulAdd(MF4 x, MF4x2 y, MF2 a) {
    MF2 result = a;
    result = mad(x.x, y._m00_m01, result);
    result = mad(x.y, y._m10_m11, result);
    result = mad(x.z, y._m20_m21, result);
    result = mad(x.w, y._m30_m31, result);
    return result;
}
MF3 MulAdd(MF4 x, MF4x3 y, MF3 a) {
    MF3 result = a;
    result = mad(x.x, y._m00_m01_m02, result);
    result = mad(x.y, y._m10_m11_m12, result);
    result = mad(x.z, y._m20_m21_m22, result);
    result = mad(x.w, y._m30_m31_m32, result);
    return result;
}
MF4 MulAdd(MF4 x, MF4x4 y, MF4 a) {
    MF4 result = a;
    result = mad(x.x, y._m00_m01_m02_m03, result);
    result = mad(x.y, y._m10_m11_m12_m13, result);
    result = mad(x.z, y._m20_m21_m22_m23, result);
    result = mad(x.w, y._m30_m31_m32_m33, result);
    return result;
}
";

fn generate_compute_style_entry(
    pass_index: u32,
    block_size: [u32; 2],
    num_threads: [u32; 3],
) -> String {
    let block_start = block_start_expr(block_size);
    format!(
        r"[numthreads({threads_x}, {threads_y}, {threads_z})]
void __M(uint3 tid : SV_GroupThreadID, uint3 gid : SV_GroupID) {{
    Pass{pass_index}({block_start}, tid);
}}
",
        threads_x = num_threads[0],
        threads_y = num_threads[1],
        threads_z = num_threads[2],
    )
}

fn generate_pixel_style_entry(
    pass_index: u32,
    pass: &MagpiePassPlan,
    pass_count: usize,
) -> UpscaleResult<String> {
    let output = pass
        .outputs
        .first()
        .ok_or_else(|| invalid_pipeline("PS-style Magpie pass requires an output texture"))?;
    if pass.outputs.len() > 1 {
        return Ok(generate_multi_output_pixel_style_entry(pass_index, pass));
    }
    let (output_size, output_pt) = pixel_style_output_symbols(pass_index, pass_count);
    Ok(format!(
        r"[numthreads(64, 1, 1)]
void __M(uint3 tid : SV_GroupThreadID, uint3 gid : SV_GroupID) {{
    uint2 gxy = (gid.xy << 4u) + Rmp8x8(tid.x);
    if (gxy.x >= {output_size}.x || gxy.y >= {output_size}.y) {{
        return;
    }}
    float2 pos = (gxy + 0.5f) * {output_pt};
    float2 step = 8 * {output_pt};

    {output}[gxy] = Pass{pass_index}(pos);

    gxy.x += 8u;
    pos.x += step.x;
    if (gxy.x < {output_size}.x && gxy.y < {output_size}.y) {{
        {output}[gxy] = Pass{pass_index}(pos);
    }}

    gxy.y += 8u;
    pos.y += step.y;
    if (gxy.x < {output_size}.x && gxy.y < {output_size}.y) {{
        {output}[gxy] = Pass{pass_index}(pos);
    }}

    gxy.x -= 8u;
    pos.x -= step.x;
    if (gxy.x < {output_size}.x && gxy.y < {output_size}.y) {{
        {output}[gxy] = Pass{pass_index}(pos);
    }}
}}
"
    ))
}

fn generate_multi_output_pixel_style_entry(pass_index: u32, pass: &MagpiePassPlan) -> String {
    let mut declarations = String::new();
    for index in 0..pass.outputs.len() {
        writeln!(&mut declarations, "    MF4 c{index};").expect("writing to String cannot fail");
    }
    let arguments = (0..pass.outputs.len())
        .map(|index| format!("c{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut stores = String::new();
    for (index, output) in pass.outputs.iter().enumerate() {
        writeln!(&mut stores, "        {output}[gxy] = c{index};")
            .expect("writing to String cannot fail");
    }
    format!(
        r"[numthreads(64, 1, 1)]
void __M(uint3 tid : SV_GroupThreadID, uint3 gid : SV_GroupID) {{
    uint2 gxy = (gid.xy << 4u) + Rmp8x8(tid.x);
    if (gxy.x >= __pass{pass_index}OutputSize.x || gxy.y >= __pass{pass_index}OutputSize.y) {{
        return;
    }}
    float2 pos = (gxy + 0.5f) * __pass{pass_index}OutputPt;
    float2 step = 8 * __pass{pass_index}OutputPt;

{declarations}    Pass{pass_index}(pos, {arguments});
{stores}

    gxy.x += 8u;
    pos.x += step.x;
    if (gxy.x < __pass{pass_index}OutputSize.x && gxy.y < __pass{pass_index}OutputSize.y) {{
        Pass{pass_index}(pos, {arguments});
{stores}    }}

    gxy.y += 8u;
    pos.y += step.y;
    if (gxy.x < __pass{pass_index}OutputSize.x && gxy.y < __pass{pass_index}OutputSize.y) {{
        Pass{pass_index}(pos, {arguments});
{stores}    }}

    gxy.x -= 8u;
    pos.x -= step.x;
    if (gxy.x < __pass{pass_index}OutputSize.x && gxy.y < __pass{pass_index}OutputSize.y) {{
        Pass{pass_index}(pos, {arguments});
{stores}    }}
}}
"
    )
}

fn pixel_style_output_symbols(pass_index: u32, pass_count: usize) -> (String, String) {
    if usize::try_from(pass_index).ok() == Some(pass_count) {
        ("__outputSize".to_owned(), "__outputPt".to_owned())
    } else {
        (
            format!("__pass{pass_index}OutputSize"),
            format!("__pass{pass_index}OutputPt"),
        )
    }
}

fn block_start_expr(block_size: [u32; 2]) -> String {
    if block_size[0] == block_size[1]
        && block_size[0].is_power_of_two()
        && let Some(shift) = checked_log2(block_size[0])
    {
        format!("(gid.xy << {shift}u)")
    } else {
        format!("gid.xy * uint2({}, {})", block_size[0], block_size[1])
    }
}

fn checked_log2(value: u32) -> Option<u32> {
    value.is_power_of_two().then_some(value.trailing_zeros())
}

fn normalized_block_size(pass: &MagpiePassPlan) -> UpscaleResult<[u32; 2]> {
    if pass.style == MagpieFxPassStyle::PixelShader {
        if pass.block_size.is_some() {
            return Err(invalid_pipeline(
                "PS-style Magpie pass must not specify BLOCK_SIZE",
            ));
        }
        return Ok([16, 16]);
    }
    let block_size = pass
        .block_size
        .as_deref()
        .ok_or_else(|| invalid_pipeline("CS-style Magpie pass requires BLOCK_SIZE"))?;
    match block_size {
        [value] if *value > 0 => Ok([*value, *value]),
        [width, height] if *width > 0 && *height > 0 => Ok([*width, *height]),
        _ => Err(invalid_pipeline("invalid Magpie BLOCK_SIZE")),
    }
}

fn normalized_num_threads(pass: &MagpiePassPlan) -> UpscaleResult<[u32; 3]> {
    if pass.style == MagpieFxPassStyle::PixelShader {
        if pass.num_threads.is_some() {
            return Err(invalid_pipeline(
                "PS-style Magpie pass must not specify NUM_THREADS",
            ));
        }
        return Ok([64, 1, 1]);
    }
    let num_threads = pass
        .num_threads
        .as_deref()
        .ok_or_else(|| invalid_pipeline("CS-style Magpie pass requires NUM_THREADS"))?;
    match num_threads {
        [x] if *x > 0 => Ok([*x, 1, 1]),
        [x, y] if *x > 0 && *y > 0 => Ok([*x, *y, 1]),
        [x, y, z] if *x > 0 && *y > 0 && *z > 0 => Ok([*x, *y, *z]),
        _ => Err(invalid_pipeline("invalid Magpie NUM_THREADS")),
    }
}

fn extend_float_macros(macros: &mut Vec<MagpieShaderMacro>, fp16: bool) {
    if fp16 {
        macros.push(MagpieShaderMacro::define("MP_FP16"));
        macros.push(MagpieShaderMacro::value("MF", "min16float"));
        extend_numeric_float_macros(macros, "min16float");
    } else {
        macros.push(MagpieShaderMacro::value("MF", "float"));
        extend_numeric_float_macros(macros, "float");
    }
}

fn extend_numeric_float_macros(macros: &mut Vec<MagpieShaderMacro>, scalar: &str) {
    (1..=4).for_each(|columns| {
        macros.push(MagpieShaderMacro::value(
            format!("MF{columns}"),
            format!("{scalar}{columns}"),
        ));
        (1..=4).for_each(|rows| {
            macros.push(MagpieShaderMacro::value(
                format!("MF{columns}x{rows}"),
                format!("{scalar}{columns}x{rows}"),
            ));
        });
    });
}

fn invalid_pipeline(message: impl Into<String>) -> UpscaleError {
    UpscaleError::InvalidPipeline(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::MagpieEffectPackage;
    use crate::parse_magpiefx;
    use crate::plan::MagpieRenderPlan;
    use borderless_upscale_core::FrameSize;
    use std::path::PathBuf;

    #[test]
    fn plans_pixel_style_pass_as_magpie_compute_entry() {
        let package = package_from_source(
            r"
//!MAGPIE EFFECT
//!VERSION 4
//!TEXTURE
Texture2D INPUT;
//!TEXTURE
Texture2D OUTPUT;
//!SAMPLER
SamplerState LINEAR;
//!PASS 1
//!STYLE PS
//!IN INPUT
//!OUT OUTPUT
float4 Pass1(float2 pos) { return 1; }
",
        );

        let plan = MagpieCompilePlan::from_package(&package).unwrap();

        assert_eq!(plan.jobs.len(), 1);
        assert_eq!(plan.jobs[0].entry_point, "__M");
        assert_eq!(plan.jobs[0].target_profile, "cs_5_0");
        assert_eq!(plan.jobs[0].block_size, [16, 16]);
        assert_eq!(plan.jobs[0].num_threads, [64, 1, 1]);
        assert!(
            plan.jobs[0]
                .macros
                .iter()
                .any(|shader_macro| shader_macro.name == "MP_PS_STYLE")
        );
        assert!(plan.jobs[0].generated_source.contains("void __M"));
        assert!(plan.jobs[0].generated_source.contains("Pass1(pos)"));
        assert!(
            plan.jobs[0]
                .generated_source
                .contains("cbuffer __CB1 : register(b0)")
        );
        assert!(
            plan.jobs[0]
                .generated_source
                .contains("Texture2D<MF4> INPUT : register(t0);")
        );
        assert!(
            plan.jobs[0]
                .generated_source
                .contains("RWTexture2D<unorm MF4> OUTPUT : register(u0);")
        );
        assert!(
            plan.jobs[0]
                .generated_source
                .contains("SamplerState LINEAR : register(s0);")
        );
        assert!(
            !plan.jobs[0]
                .generated_source
                .contains("Texture2D INPUT;\n//!TEXTURE")
        );
    }

    #[test]
    fn plans_compute_pass_threads_and_fp16_macros() {
        let package = package_from_source(
            r"
//!MAGPIE EFFECT
//!VERSION 4
//!USE MulAdd, _DYNAMIC
//!CAPABILITY FP16
//!PARAMETER
//!LABEL Sharpness
//!DEFAULT 1
//!MIN 0
//!MAX 2
//!STEP 0.1
float sharpness;
//!PARAMETER
//!LABEL Mode
//!DEFAULT 1
//!MIN 0
//!MAX 3
//!STEP 1
int mode;
//!TEXTURE
Texture2D INPUT;
//!TEXTURE
Texture2D OUTPUT;
//!PASS 1
//!DESC Setup
//!IN INPUT
//!OUT OUTPUT
//!BLOCK_SIZE 16, 8
//!NUM_THREADS 64
void Pass1(uint2 blockStart, uint3 threadId) {}
",
        );

        let plan = MagpieCompilePlan::from_package(&package).unwrap();
        let job = &plan.jobs[0];

        assert_eq!(job.pass_name, "Setup");
        assert_eq!(job.block_size, [16, 8]);
        assert_eq!(job.num_threads, [64, 1, 1]);
        assert!(job.macros.iter().any(|shader_macro| {
            shader_macro.name == "MF" && shader_macro.value.as_deref() == Some("min16float")
        }));
        assert!(job.generated_source.contains("[numthreads(64, 1, 1)]"));
        assert!(
            job.generated_source
                .contains("Pass1(gid.xy * uint2(16, 8), tid)")
        );
        assert!(job.generated_source.contains("float sharpness;"));
        assert!(job.generated_source.contains("int mode;"));
        assert!(
            job.generated_source
                .contains("cbuffer __CB2 : register(b1) { uint __frameCount; };")
        );
        assert!(
            job.generated_source
                .contains("MF4 MulAdd(MF4 x, MF4x4 y, MF4 a)")
        );
        assert_eq!(job.parameter_bindings.len(), 2);
        assert_eq!(
            job.parameter_bindings[0].value_type,
            MagpieFxParameterType::Float
        );
        assert_eq!(
            job.parameter_bindings[1].value_type,
            MagpieFxParameterType::Int
        );
    }

    fn package_from_source(source: &str) -> MagpieEffectPackage {
        let effect = parse_magpiefx(source).unwrap();
        let input_size = FrameSize::new(640, 480).unwrap();
        let output_size = FrameSize::new(1280, 960).unwrap();
        let render_plan = MagpieRenderPlan::from_effect(&effect, input_size, output_size).unwrap();
        let compiler_prelude_source = effect.prelude_source.clone();
        let compiler_common_source = effect.common_source.clone();
        let compiler_pass_sources = effect
            .passes
            .iter()
            .map(|pass| pass.source.clone())
            .collect();
        MagpieEffectPackage {
            effect_path: PathBuf::from("effect.hlsl"),
            effect,
            render_plan,
            compiler_source: source.to_owned(),
            compiler_prelude_source,
            compiler_common_source,
            compiler_pass_sources,
            includes: Vec::new(),
            source_assets: Vec::new(),
        }
    }
}
