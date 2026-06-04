// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: Copyright (c) Xu and Magpie contributors
// SPDX-FileCopyrightText: Copyright (c) Borderless Oxide contributors
// Magpie-compatible HLSL compiler handoff for the GPL port:
// https://github.com/Blinue/Magpie

use crate::compiler::MagpieShaderJob;
use borderless_upscale_core::{UpscaleError, UpscaleResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MagpieHlslCompilerKind {
    #[default]
    Fxc,
    Dxc,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieHlslCompilerInvocation {
    pub executable: PathBuf,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagpieCompiledShader {
    pub pass_index: u32,
    pub pass_name: String,
    pub entry_point: String,
    pub target_profile: String,
    pub bytecode: Vec<u8>,
    pub diagnostics: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MagpieExternalHlslCompiler {
    executable: PathBuf,
    kind: MagpieHlslCompilerKind,
}

impl MagpieExternalHlslCompiler {
    #[must_use]
    pub fn fxc(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            kind: MagpieHlslCompilerKind::Fxc,
        }
    }

    #[must_use]
    pub fn dxc(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            kind: MagpieHlslCompilerKind::Dxc,
        }
    }

    #[must_use]
    pub fn executable(&self) -> &Path {
        &self.executable
    }

    #[must_use]
    pub const fn kind(&self) -> MagpieHlslCompilerKind {
        self.kind
    }

    #[must_use]
    pub fn invocation_for_paths(
        &self,
        job: &MagpieShaderJob,
        source_path: impl AsRef<Path>,
        output_path: impl AsRef<Path>,
    ) -> MagpieHlslCompilerInvocation {
        let source_path = source_path.as_ref().display().to_string();
        let output_path = output_path.as_ref().display().to_string();
        let args = match self.kind {
            MagpieHlslCompilerKind::Fxc => vec![
                "/nologo".to_owned(),
                "/T".to_owned(),
                job.target_profile.clone(),
                "/E".to_owned(),
                job.entry_point.clone(),
                "/Fo".to_owned(),
                output_path,
                source_path,
            ],
            MagpieHlslCompilerKind::Dxc => vec![
                "-nologo".to_owned(),
                "-T".to_owned(),
                job.target_profile.clone(),
                "-E".to_owned(),
                job.entry_point.clone(),
                "-Fo".to_owned(),
                output_path,
                source_path,
            ],
        };
        MagpieHlslCompilerInvocation {
            executable: self.executable.clone(),
            args,
        }
    }

    pub fn compile_plan(
        &self,
        jobs: &[MagpieShaderJob],
    ) -> UpscaleResult<Vec<MagpieCompiledShader>> {
        jobs.iter().map(|job| self.compile_job(job)).collect()
    }

    pub fn compile_job(&self, job: &MagpieShaderJob) -> UpscaleResult<MagpieCompiledShader> {
        let temp = TempShaderFiles::new(job.pass_index)?;
        fs::write(&temp.source_path, &job.generated_source).map_err(|err| {
            unavailable(format!(
                "failed to write temporary HLSL source {}: {err}",
                temp.source_path.display()
            ))
        })?;

        let invocation = self.invocation_for_paths(job, &temp.source_path, &temp.output_path);
        let command_output = Command::new(&invocation.executable)
            .args(&invocation.args)
            .output()
            .map_err(|err| {
                unavailable(format!(
                    "failed to run HLSL compiler {}: {err}",
                    invocation.executable.display()
                ))
            })?;
        let diagnostics = compiler_diagnostics(&command_output.stdout, &command_output.stderr);
        if !command_output.status.success() {
            return Err(unavailable(format!(
                "HLSL compiler failed for pass {} with status {}: {}",
                job.pass_index, command_output.status, diagnostics
            )));
        }

        let bytecode = fs::read(&temp.output_path).map_err(|err| {
            unavailable(format!(
                "failed to read compiled shader {}: {err}",
                temp.output_path.display()
            ))
        })?;

        Ok(MagpieCompiledShader {
            pass_index: job.pass_index,
            pass_name: job.pass_name.clone(),
            entry_point: job.entry_point.clone(),
            target_profile: job.target_profile.clone(),
            bytecode,
            diagnostics,
        })
    }
}

struct TempShaderFiles {
    source_path: PathBuf,
    output_path: PathBuf,
}

impl TempShaderFiles {
    fn new(pass_index: u32) -> UpscaleResult<Self> {
        let unique = unique_temp_name(pass_index)?;
        let temp_dir = std::env::temp_dir();
        Ok(Self {
            source_path: temp_dir.join(format!("{unique}.hlsl")),
            output_path: temp_dir.join(format!("{unique}.cso")),
        })
    }
}

impl Drop for TempShaderFiles {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.source_path);
        let _ = fs::remove_file(&self.output_path);
    }
}

fn unique_temp_name(pass_index: u32) -> UpscaleResult<String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| unavailable(format!("system clock is before Unix epoch: {err}")))?
        .as_nanos();
    Ok(format!(
        "borderless-magpie-{}-{timestamp}-pass{pass_index}",
        std::process::id()
    ))
}

fn compiler_diagnostics(stdout: &[u8], stderr: &[u8]) -> String {
    [stdout, stderr]
        .into_iter()
        .map(String::from_utf8_lossy)
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn unavailable(message: impl Into<String>) -> UpscaleError {
    UpscaleError::BackendUnavailable(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::{
        MAGPIE_ENTRY_POINT, MAGPIE_TARGET_PROFILE, MagpieShaderJob, MagpieShaderMacro,
    };
    use crate::magpiefx::MagpieFxPassStyle;

    #[test]
    fn builds_fxc_invocation_for_magpie_compute_shader() {
        let compiler = MagpieExternalHlslCompiler::fxc("fxc.exe");
        let job = shader_job();
        let invocation = compiler.invocation_for_paths(&job, "pass1.hlsl", "pass1.cso");

        assert_eq!(invocation.executable, PathBuf::from("fxc.exe"));
        assert_eq!(
            invocation.args,
            [
                "/nologo",
                "/T",
                MAGPIE_TARGET_PROFILE,
                "/E",
                MAGPIE_ENTRY_POINT,
                "/Fo",
                "pass1.cso",
                "pass1.hlsl"
            ]
        );
    }

    #[test]
    fn reports_missing_compiler_as_backend_unavailable() {
        let compiler = MagpieExternalHlslCompiler::fxc(
            std::env::temp_dir().join("borderless-magpie-missing-fxc.exe"),
        );
        let err = compiler.compile_job(&shader_job()).unwrap_err();

        assert!(matches!(err, UpscaleError::BackendUnavailable(_)));
    }

    fn shader_job() -> MagpieShaderJob {
        MagpieShaderJob {
            pass_index: 1,
            pass_name: "Pass 1".to_owned(),
            pass_function: "Pass1".to_owned(),
            entry_point: MAGPIE_ENTRY_POINT.to_owned(),
            target_profile: MAGPIE_TARGET_PROFILE.to_owned(),
            style: MagpieFxPassStyle::Compute,
            block_size: [8, 8],
            num_threads: [64, 1, 1],
            inputs: Vec::new(),
            outputs: Vec::new(),
            texture_bindings: Vec::new(),
            sampler_bindings: Vec::new(),
            parameter_bindings: Vec::new(),
            inline_parameters: Vec::new(),
            base_source: "void Pass1(uint2 blockStart, uint3 threadId) {}".to_owned(),
            generated_source: "void Pass1(uint2 blockStart, uint3 threadId) {}\n[numthreads(64, 1, 1)]\nvoid __M(uint3 tid : SV_GroupThreadID, uint3 gid : SV_GroupID) { Pass1(gid.xy, tid); }".to_owned(),
            macros: vec![MagpieShaderMacro::value("MF", "float")],
        }
    }
}
