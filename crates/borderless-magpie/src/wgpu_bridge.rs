// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: Copyright (c) Xu and Magpie contributors
// SPDX-FileCopyrightText: Copyright (c) Borderless Oxide contributors
// Magpie-compatible wgpu descriptor bridge for the GPL comparison path:
// https://github.com/Blinue/Magpie

use crate::backend::{
    MagpieBackendBufferDescriptor, MagpieBackendDescriptorPlan, MagpieBackendSamplerDescriptor,
    MagpieBackendTextureBind, MagpieBackendTextureDescriptor,
};
use crate::formats::MagpieTextureComponent;
use crate::magpiefx::{MagpieFxPassStyle, MagpieFxSamplerAddress, MagpieFxSamplerFilter};
use crate::package::MagpieEffectPackage;
use crate::plan::MagpieTextureFormat;
use crate::resources::{
    MagpieConstantBufferBinding, MagpieResourcePass, MagpieResourcePlan,
    MagpieSamplerResourceBinding, MagpieTextureResourceBinding,
};
use crate::upload::{MagpieSourceUpload, MagpieSourceUploadPlan};
use borderless_upscale_core::{UpscaleError, UpscaleResult};
use std::borrow::Cow;

pub const MAGPIE_WGPU_CONSTANT_BINDING_BASE: u32 = 0;
pub const MAGPIE_WGPU_SHADER_RESOURCE_BINDING_BASE: u32 = 32;
pub const MAGPIE_WGPU_UNORDERED_ACCESS_BINDING_BASE: u32 = 64;
pub const MAGPIE_WGPU_SAMPLER_BINDING_BASE: u32 = 96;

pub struct MagpieWgpuDescriptorPlan<'a> {
    pub textures: Vec<wgpu::TextureDescriptor<'a>>,
    pub buffers: Vec<wgpu::BufferDescriptor<'a>>,
    pub samplers: Vec<wgpu::SamplerDescriptor<'a>>,
}

impl<'a> MagpieWgpuDescriptorPlan<'a> {
    pub fn from_backend_descriptors(
        descriptors: &'a MagpieBackendDescriptorPlan,
    ) -> UpscaleResult<Self> {
        let textures = descriptors
            .textures
            .iter()
            .map(wgpu_texture_descriptor)
            .collect::<UpscaleResult<Vec<_>>>()?;
        let buffers = descriptors
            .buffers
            .iter()
            .map(wgpu_buffer_descriptor)
            .collect();
        let samplers = descriptors
            .samplers
            .iter()
            .map(wgpu_sampler_descriptor)
            .collect();

        Ok(Self {
            textures,
            buffers,
            samplers,
        })
    }
}

#[derive(Clone, Debug)]
pub struct MagpieWgpuPreparedEffectPlan {
    pub resources: MagpieResourcePlan,
    pub backend_descriptors: MagpieBackendDescriptorPlan,
    pub source_uploads: MagpieWgpuSourceUploadPlan,
    pub binding_layouts: MagpieWgpuBindingLayoutPlan,
    pub declarations: MagpieWgpuDeclarationPlan,
    pub shaders: MagpieWgpuShaderPlan,
    pub execution: MagpieWgpuExecutionPlan,
}

impl MagpieWgpuPreparedEffectPlan {
    pub fn from_package(package: &MagpieEffectPackage) -> UpscaleResult<Self> {
        let resources = MagpieResourcePlan::from_package(package)?;
        Self::from_package_and_resource_plan(package, resources)
    }

    pub fn from_package_and_resource_plan(
        package: &MagpieEffectPackage,
        resources: MagpieResourcePlan,
    ) -> UpscaleResult<Self> {
        let backend_descriptors = MagpieBackendDescriptorPlan::from_resource_plan(&resources)?;
        let source_uploads = MagpieWgpuSourceUploadPlan::from_resource_plan(&resources)?;
        let binding_layouts = MagpieWgpuBindingLayoutPlan::from_resource_plan(&resources)?;
        let declarations = MagpieWgpuDeclarationPlan::from_resource_plan(&resources)?;
        let translated =
            MagpieWgpuBodyTranslationPlan::from_package_and_resource_plan(package, &resources)?;
        let shaders = MagpieWgpuShaderPlan::from_wgsl_passes(&binding_layouts, translated.passes)?;
        let execution = MagpieWgpuExecutionPlan::from_resource_plan(&resources);

        Ok(Self {
            resources,
            backend_descriptors,
            source_uploads,
            binding_layouts,
            declarations,
            shaders,
            execution,
        })
    }

    pub fn descriptor_plan(&self) -> UpscaleResult<MagpieWgpuDescriptorPlan<'_>> {
        MagpieWgpuDescriptorPlan::from_backend_descriptors(&self.backend_descriptors)
    }

    pub fn create_runtime_objects(
        &self,
        device: &wgpu::Device,
    ) -> UpscaleResult<MagpieWgpuRuntimeObjects> {
        let resources =
            MagpieWgpuResourceObjects::from_backend_descriptors(device, &self.backend_descriptors)?;
        let layouts = self.binding_layouts.create_pass_layout_objects(device);
        let bind_groups = self
            .binding_layouts
            .passes
            .iter()
            .zip(layouts.iter())
            .map(|(layout, objects)| {
                resources.create_pass_bind_group(device, layout, &objects.bind_group_layout)
            })
            .collect::<UpscaleResult<Vec<_>>>()?;
        let pipelines = self.shaders.create_pipeline_objects(device, &layouts)?;

        Ok(MagpieWgpuRuntimeObjects {
            resources,
            layouts,
            bind_groups,
            pipelines,
        })
    }

    pub fn write_source_uploads(
        &self,
        queue: &wgpu::Queue,
        runtime: &MagpieWgpuRuntimeObjects,
    ) -> UpscaleResult<()> {
        runtime.write_source_uploads(queue, &self.source_uploads)
    }

    pub fn record_execution(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        runtime: &MagpieWgpuRuntimeObjects,
    ) -> UpscaleResult<()> {
        runtime.record_execution(encoder, &self.execution)
    }

    pub fn write_sources_and_record_execution(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        runtime: &MagpieWgpuRuntimeObjects,
    ) -> UpscaleResult<()> {
        self.write_source_uploads(queue, runtime)?;
        self.record_execution(encoder, runtime)
    }
}

#[derive(Debug)]
pub struct MagpieWgpuRuntimeObjects {
    pub resources: MagpieWgpuResourceObjects,
    pub layouts: Vec<MagpieWgpuPassLayoutObjects>,
    pub bind_groups: Vec<MagpieWgpuPassBindGroup>,
    pub pipelines: Vec<MagpieWgpuPipelineObjects>,
}

impl MagpieWgpuRuntimeObjects {
    pub fn write_source_uploads(
        &self,
        queue: &wgpu::Queue,
        uploads: &MagpieWgpuSourceUploadPlan,
    ) -> UpscaleResult<()> {
        self.resources.write_source_uploads(queue, uploads)
    }

    pub fn record_execution(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        execution: &MagpieWgpuExecutionPlan,
    ) -> UpscaleResult<()> {
        execution.record_compute_pass(encoder, &self.pipelines, &self.bind_groups)
    }
}

#[derive(Debug)]
pub struct MagpieWgpuResourceObjects {
    pub textures: Vec<MagpieWgpuTextureObject>,
    pub buffers: Vec<MagpieWgpuBufferObject>,
    pub samplers: Vec<MagpieWgpuSamplerObject>,
}

impl MagpieWgpuResourceObjects {
    pub fn from_backend_descriptors(
        device: &wgpu::Device,
        descriptors: &MagpieBackendDescriptorPlan,
    ) -> UpscaleResult<Self> {
        let textures = descriptors
            .textures
            .iter()
            .map(|texture| create_texture_object(device, texture))
            .collect::<UpscaleResult<Vec<_>>>()?;
        let buffers = descriptors
            .buffers
            .iter()
            .map(|buffer| create_buffer_object(device, buffer))
            .collect::<UpscaleResult<Vec<_>>>()?;
        let samplers = descriptors
            .samplers
            .iter()
            .map(|sampler| create_sampler_object(device, sampler))
            .collect::<Vec<_>>();

        Ok(Self {
            textures,
            buffers,
            samplers,
        })
    }

    pub fn create_pass_bind_group(
        &self,
        device: &wgpu::Device,
        layout: &MagpieWgpuPassLayout,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> UpscaleResult<MagpieWgpuPassBindGroup> {
        let entries = layout
            .bindings
            .iter()
            .map(|binding| self.bind_group_entry(binding))
            .collect::<UpscaleResult<Vec<_>>>()?;
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(layout.pass_name.as_str()),
            layout: bind_group_layout,
            entries: &entries,
        });

        Ok(MagpieWgpuPassBindGroup {
            pass_index: layout.pass_index,
            pass_name: layout.pass_name.clone(),
            bind_group,
        })
    }

    #[must_use]
    pub fn texture(&self, name: &str) -> Option<&MagpieWgpuTextureObject> {
        self.textures.iter().find(|texture| texture.name == name)
    }

    #[must_use]
    pub fn buffer(&self, name: &str) -> Option<&MagpieWgpuBufferObject> {
        self.buffers.iter().find(|buffer| buffer.name == name)
    }

    #[must_use]
    pub fn sampler(&self, name: &str) -> Option<&MagpieWgpuSamplerObject> {
        self.samplers.iter().find(|sampler| sampler.name == name)
    }

    fn bind_group_entry<'a>(
        &'a self,
        binding: &MagpieWgpuBinding,
    ) -> UpscaleResult<wgpu::BindGroupEntry<'a>> {
        Ok(wgpu::BindGroupEntry {
            binding: binding.binding,
            resource: self.binding_resource(binding)?,
        })
    }

    pub fn write_source_uploads(
        &self,
        queue: &wgpu::Queue,
        uploads: &MagpieWgpuSourceUploadPlan,
    ) -> UpscaleResult<()> {
        uploads
            .uploads
            .iter()
            .try_for_each(|upload| upload.write_to_queue(queue, self))
    }

    fn binding_resource<'a>(
        &'a self,
        binding: &MagpieWgpuBinding,
    ) -> UpscaleResult<wgpu::BindingResource<'a>> {
        match binding.kind {
            MagpieWgpuBindingKind::ConstantBuffer => {
                let buffer = self.buffer(&binding.name).ok_or_else(|| {
                    invalid_pipeline(format!("missing wgpu buffer {}", binding.name))
                })?;
                Ok(wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &buffer.buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(buffer.byte_len as u64),
                }))
            }
            MagpieWgpuBindingKind::ShaderResource | MagpieWgpuBindingKind::UnorderedAccess => {
                let texture = self.texture(&binding.name).ok_or_else(|| {
                    invalid_pipeline(format!("missing wgpu texture {}", binding.name))
                })?;
                Ok(wgpu::BindingResource::TextureView(&texture.view))
            }
            MagpieWgpuBindingKind::Sampler => {
                let sampler = self.sampler(&binding.name).ok_or_else(|| {
                    invalid_pipeline(format!("missing wgpu sampler {}", binding.name))
                })?;
                Ok(wgpu::BindingResource::Sampler(&sampler.sampler))
            }
        }
    }
}

#[derive(Debug)]
pub struct MagpieWgpuTextureObject {
    pub name: String,
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
}

#[derive(Debug)]
pub struct MagpieWgpuBufferObject {
    pub name: String,
    pub byte_len: usize,
    pub buffer: wgpu::Buffer,
}

#[derive(Debug)]
pub struct MagpieWgpuSamplerObject {
    pub name: String,
    pub sampler: wgpu::Sampler,
}

#[derive(Clone, Debug)]
pub struct MagpieWgpuPassBindGroup {
    pub pass_index: u32,
    pub pass_name: String,
    pub bind_group: wgpu::BindGroup,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MagpieWgpuShaderPlan {
    pub passes: Vec<MagpieWgpuShaderPass>,
}

impl MagpieWgpuShaderPlan {
    pub fn from_wgsl_passes(
        layouts: &MagpieWgpuBindingLayoutPlan,
        passes: Vec<MagpieWgpuShaderPass>,
    ) -> UpscaleResult<Self> {
        passes
            .iter()
            .try_for_each(|pass| validate_wgsl_pass(layouts, pass))?;

        Ok(Self { passes })
    }

    #[must_use]
    pub fn pass(&self, pass_index: u32) -> Option<&MagpieWgpuShaderPass> {
        self.passes
            .iter()
            .find(|pass| pass.pass_index == pass_index)
    }

    pub fn create_pipeline_objects(
        &self,
        device: &wgpu::Device,
        layouts: &[MagpieWgpuPassLayoutObjects],
    ) -> UpscaleResult<Vec<MagpieWgpuPipelineObjects>> {
        self.passes
            .iter()
            .map(|pass| pass.create_pipeline_objects(device, layouts))
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MagpieWgpuShaderPass {
    pub pass_index: u32,
    pub pass_name: String,
    pub entry_point: String,
    pub wgsl_source: String,
}

impl MagpieWgpuShaderPass {
    #[must_use]
    pub fn new(
        pass_index: u32,
        pass_name: impl Into<String>,
        entry_point: impl Into<String>,
        wgsl_source: impl Into<String>,
    ) -> Self {
        Self {
            pass_index,
            pass_name: pass_name.into(),
            entry_point: entry_point.into(),
            wgsl_source: wgsl_source.into(),
        }
    }

    #[must_use]
    pub fn shader_module_descriptor(&self) -> wgpu::ShaderModuleDescriptor<'_> {
        wgpu::ShaderModuleDescriptor {
            label: Some(self.pass_name.as_str()),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(self.wgsl_source.as_str())),
        }
    }

    pub fn create_pipeline_objects(
        &self,
        device: &wgpu::Device,
        layouts: &[MagpieWgpuPassLayoutObjects],
    ) -> UpscaleResult<MagpieWgpuPipelineObjects> {
        let layout = layout_object_by_pass(layouts, self.pass_index, &self.pass_name)?;
        let shader_module = device.create_shader_module(self.shader_module_descriptor());
        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(self.pass_name.as_str()),
            layout: Some(&layout.pipeline_layout),
            module: &shader_module,
            entry_point: Some(self.entry_point.as_str()),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        Ok(MagpieWgpuPipelineObjects {
            pass_index: self.pass_index,
            pass_name: self.pass_name.clone(),
            shader_module,
            compute_pipeline,
        })
    }
}

#[derive(Clone, Debug)]
pub struct MagpieWgpuPipelineObjects {
    pub pass_index: u32,
    pub pass_name: String,
    pub shader_module: wgpu::ShaderModule,
    pub compute_pipeline: wgpu::ComputePipeline,
}

#[derive(Clone, Debug)]
pub struct MagpieWgpuSourceUploadPlan {
    pub uploads: Vec<MagpieWgpuSourceUpload>,
}

impl MagpieWgpuSourceUploadPlan {
    pub fn from_resource_plan(resources: &MagpieResourcePlan) -> UpscaleResult<Self> {
        let plan = MagpieSourceUploadPlan {
            uploads: resources.source_uploads.clone(),
        };
        Self::from_source_upload_plan(&plan)
    }

    pub fn from_source_upload_plan(plan: &MagpieSourceUploadPlan) -> UpscaleResult<Self> {
        let uploads = plan
            .uploads
            .iter()
            .map(MagpieWgpuSourceUpload::from_source_upload)
            .collect::<UpscaleResult<Vec<_>>>()?;

        Ok(Self { uploads })
    }

    #[must_use]
    pub fn upload(&self, texture_name: &str) -> Option<&MagpieWgpuSourceUpload> {
        self.uploads
            .iter()
            .find(|upload| upload.texture_name == texture_name)
    }

    pub fn write_to_queue(
        &self,
        queue: &wgpu::Queue,
        resources: &MagpieWgpuResourceObjects,
    ) -> UpscaleResult<()> {
        resources.write_source_uploads(queue, self)
    }
}

#[derive(Clone, Debug)]
pub struct MagpieWgpuSourceUpload {
    pub texture_name: String,
    pub path: std::path::PathBuf,
    pub data_offset: usize,
    pub data_byte_len: usize,
    pub layout: wgpu::TexelCopyBufferLayout,
    pub size: wgpu::Extent3d,
}

impl MagpieWgpuSourceUpload {
    pub fn from_source_upload(upload: &MagpieSourceUpload) -> UpscaleResult<Self> {
        let data_byte_len = usize::try_from(upload.byte_len).map_err(|_| {
            invalid_pipeline(format!(
                "source texture {} upload byte length does not fit usize",
                upload.texture_name
            ))
        })?;

        Ok(Self {
            texture_name: upload.texture_name.clone(),
            path: upload.path.clone(),
            data_offset: upload.data_offset,
            data_byte_len,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(upload.row_pitch),
                rows_per_image: Some(upload.size.height),
            },
            size: wgpu::Extent3d {
                width: upload.size.width,
                height: upload.size.height,
                depth_or_array_layers: 1,
            },
        })
    }

    pub fn read_data(&self) -> UpscaleResult<Vec<u8>> {
        let bytes = std::fs::read(&self.path).map_err(|err| {
            UpscaleError::BackendUnavailable(format!(
                "failed to read source texture {}: {err}",
                self.path.display()
            ))
        })?;
        let end = self
            .data_offset
            .checked_add(self.data_byte_len)
            .ok_or_else(|| {
                invalid_pipeline(format!(
                    "source texture {} upload byte range overflowed",
                    self.texture_name
                ))
            })?;

        bytes
            .get(self.data_offset..end)
            .map(<[u8]>::to_vec)
            .ok_or_else(|| {
                invalid_pipeline(format!(
                    "source texture {} data is shorter than planned upload",
                    self.texture_name
                ))
            })
    }

    pub fn write_to_queue(
        &self,
        queue: &wgpu::Queue,
        resources: &MagpieWgpuResourceObjects,
    ) -> UpscaleResult<()> {
        let texture = resources.texture(&self.texture_name).ok_or_else(|| {
            invalid_pipeline(format!("missing wgpu source texture {}", self.texture_name))
        })?;
        let data = self.read_data()?;
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &data,
            self.layout,
            self.size,
        );

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MagpieWgpuExecutionPlan {
    pub passes: Vec<MagpieWgpuExecutionPass>,
}

impl MagpieWgpuExecutionPlan {
    #[must_use]
    pub fn from_resource_plan(resources: &MagpieResourcePlan) -> Self {
        let passes = resources
            .passes
            .iter()
            .map(MagpieWgpuExecutionPass::from_resource_pass)
            .collect();

        Self { passes }
    }

    #[must_use]
    pub fn pass(&self, pass_index: u32) -> Option<&MagpieWgpuExecutionPass> {
        self.passes
            .iter()
            .find(|pass| pass.pass_index == pass_index)
    }

    pub fn executable_passes<'a>(
        &'a self,
        pipelines: &'a [MagpieWgpuPipelineObjects],
        bind_groups: &'a [MagpieWgpuPassBindGroup],
    ) -> UpscaleResult<Vec<MagpieWgpuExecutablePass<'a>>> {
        self.passes
            .iter()
            .map(|pass| pass.executable_pass(pipelines, bind_groups))
            .collect()
    }

    pub fn record_compute_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        pipelines: &[MagpieWgpuPipelineObjects],
        bind_groups: &[MagpieWgpuPassBindGroup],
    ) -> UpscaleResult<()> {
        let passes = self.executable_passes(pipelines, bind_groups)?;
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Magpie wgpu execution"),
            timestamp_writes: None,
        });

        for pass in &passes {
            compute_pass.set_pipeline(pass.pipeline);
            compute_pass.set_bind_group(0, pass.bind_group, &[]);
            compute_pass.dispatch_workgroups(
                pass.group_count[0],
                pass.group_count[1],
                pass.group_count[2],
            );
        }

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MagpieWgpuExecutionPass {
    pub pass_index: u32,
    pub pass_name: String,
    pub group_count: [u32; 3],
}

impl MagpieWgpuExecutionPass {
    fn from_resource_pass(pass: &MagpieResourcePass) -> Self {
        Self {
            pass_index: pass.pass_index,
            pass_name: pass.pass_name.clone(),
            group_count: pass.dispatch.group_count,
        }
    }

    fn executable_pass<'a>(
        &'a self,
        pipelines: &'a [MagpieWgpuPipelineObjects],
        bind_groups: &'a [MagpieWgpuPassBindGroup],
    ) -> UpscaleResult<MagpieWgpuExecutablePass<'a>> {
        let pipeline = pipeline_by_pass(pipelines, self.pass_index, &self.pass_name)?;
        let bind_group = bind_group_by_pass(bind_groups, self.pass_index, &self.pass_name)?;

        Ok(MagpieWgpuExecutablePass {
            pass_index: self.pass_index,
            pass_name: self.pass_name.clone(),
            group_count: self.group_count,
            pipeline: &pipeline.compute_pipeline,
            bind_group: &bind_group.bind_group,
        })
    }
}

#[derive(Debug)]
pub struct MagpieWgpuExecutablePass<'a> {
    pub pass_index: u32,
    pub pass_name: String,
    pub group_count: [u32; 3],
    pub pipeline: &'a wgpu::ComputePipeline,
    pub bind_group: &'a wgpu::BindGroup,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MagpieWgpuDeclarationPlan {
    pub passes: Vec<MagpieWgpuDeclarationPass>,
}

impl MagpieWgpuDeclarationPlan {
    pub fn from_resource_plan(resources: &MagpieResourcePlan) -> UpscaleResult<Self> {
        let layouts = MagpieWgpuBindingLayoutPlan::from_resource_plan(resources)?;
        let passes = resources
            .passes
            .iter()
            .map(|pass| wgsl_declaration_pass(resources, pass, &layouts))
            .collect::<UpscaleResult<Vec<_>>>()?;

        Ok(Self { passes })
    }

    #[must_use]
    pub fn pass(&self, pass_index: u32) -> Option<&MagpieWgpuDeclarationPass> {
        self.passes
            .iter()
            .find(|pass| pass.pass_index == pass_index)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MagpieWgpuDeclarationPass {
    pub pass_index: u32,
    pub pass_name: String,
    pub declarations: Vec<MagpieWgpuDeclaration>,
    pub source: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MagpieWgpuDeclaration {
    pub name: String,
    pub binding: u32,
    pub kind: MagpieWgpuBindingKind,
    pub source: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MagpieWgpuBodyTranslationPlan {
    pub passes: Vec<MagpieWgpuShaderPass>,
}

impl MagpieWgpuBodyTranslationPlan {
    pub fn from_package_and_resource_plan(
        package: &MagpieEffectPackage,
        resources: &MagpieResourcePlan,
    ) -> UpscaleResult<Self> {
        let declarations = MagpieWgpuDeclarationPlan::from_resource_plan(resources)?;
        let passes = resources
            .passes
            .iter()
            .map(|pass| translate_simple_compute_pass(package, pass, &declarations))
            .collect::<UpscaleResult<Vec<_>>>()?;

        Ok(Self { passes })
    }

    #[must_use]
    pub fn shader_plan(self) -> MagpieWgpuShaderPlan {
        MagpieWgpuShaderPlan {
            passes: self.passes,
        }
    }

    #[must_use]
    pub fn pass(&self, pass_index: u32) -> Option<&MagpieWgpuShaderPass> {
        self.passes
            .iter()
            .find(|pass| pass.pass_index == pass_index)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MagpieWgpuBindingLayoutPlan {
    pub passes: Vec<MagpieWgpuPassLayout>,
}

impl MagpieWgpuBindingLayoutPlan {
    pub fn from_resource_plan(resources: &MagpieResourcePlan) -> UpscaleResult<Self> {
        let passes = resources
            .passes
            .iter()
            .map(|pass| wgpu_pass_layout(resources, pass))
            .collect::<UpscaleResult<Vec<_>>>()?;

        Ok(Self { passes })
    }

    #[must_use]
    pub fn pass(&self, pass_index: u32) -> Option<&MagpieWgpuPassLayout> {
        self.passes
            .iter()
            .find(|pass| pass.pass_index == pass_index)
    }

    #[must_use]
    pub fn create_pass_layout_objects(
        &self,
        device: &wgpu::Device,
    ) -> Vec<MagpieWgpuPassLayoutObjects> {
        self.passes
            .iter()
            .map(|pass| pass.create_layout_objects(device))
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MagpieWgpuPassLayout {
    pub pass_index: u32,
    pub pass_name: String,
    pub bindings: Vec<MagpieWgpuBinding>,
    pub entries: Vec<wgpu::BindGroupLayoutEntry>,
}

impl MagpieWgpuPassLayout {
    #[must_use]
    pub fn bind_group_layout_descriptor(&self) -> wgpu::BindGroupLayoutDescriptor<'_> {
        wgpu::BindGroupLayoutDescriptor {
            label: Some(self.pass_name.as_str()),
            entries: &self.entries,
        }
    }

    #[must_use]
    pub fn create_bind_group_layout(&self, device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&self.bind_group_layout_descriptor())
    }

    #[must_use]
    pub fn create_layout_objects(&self, device: &wgpu::Device) -> MagpieWgpuPassLayoutObjects {
        let bind_group_layout = self.create_bind_group_layout(device);
        let bind_group_layouts = [Some(&bind_group_layout)];
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(self.pass_name.as_str()),
            bind_group_layouts: &bind_group_layouts,
            immediate_size: 0,
        });

        MagpieWgpuPassLayoutObjects {
            pass_index: self.pass_index,
            pass_name: self.pass_name.clone(),
            bind_group_layout,
            pipeline_layout,
        }
    }
}

#[derive(Clone, Debug)]
pub struct MagpieWgpuPassLayoutObjects {
    pub pass_index: u32,
    pub pass_name: String,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub pipeline_layout: wgpu::PipelineLayout,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MagpieWgpuBinding {
    pub name: String,
    pub register: u32,
    pub binding: u32,
    pub kind: MagpieWgpuBindingKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MagpieWgpuBindingKind {
    ConstantBuffer,
    ShaderResource,
    UnorderedAccess,
    Sampler,
}

pub fn wgpu_texture_descriptor(
    texture: &MagpieBackendTextureDescriptor,
) -> UpscaleResult<wgpu::TextureDescriptor<'_>> {
    Ok(wgpu::TextureDescriptor {
        label: Some(texture.name.as_str()),
        size: wgpu::Extent3d {
            width: texture.size.width,
            height: texture.size.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu_texture_format(&texture.format.format)?,
        usage: wgpu_texture_usages(&texture.bind_flags),
        view_formats: &[],
    })
}

#[must_use]
pub fn wgpu_buffer_descriptor(
    buffer: &MagpieBackendBufferDescriptor,
) -> wgpu::BufferDescriptor<'_> {
    wgpu::BufferDescriptor {
        label: Some(buffer.name.as_str()),
        size: buffer.byte_len as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    }
}

#[must_use]
pub fn wgpu_sampler_descriptor(
    sampler: &MagpieBackendSamplerDescriptor,
) -> wgpu::SamplerDescriptor<'_> {
    let address = wgpu_address_mode(sampler.address);
    let filter = wgpu_filter_mode(sampler.filter);
    wgpu::SamplerDescriptor {
        label: Some(sampler.name.as_str()),
        address_mode_u: address,
        address_mode_v: address,
        address_mode_w: address,
        mag_filter: filter,
        min_filter: filter,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        lod_min_clamp: 0.0,
        lod_max_clamp: 32.0,
        compare: None,
        anisotropy_clamp: 1,
        border_color: None,
    }
}

fn create_texture_object(
    device: &wgpu::Device,
    descriptor: &MagpieBackendTextureDescriptor,
) -> UpscaleResult<MagpieWgpuTextureObject> {
    let texture = device.create_texture(&wgpu_texture_descriptor(descriptor)?);
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    Ok(MagpieWgpuTextureObject {
        name: descriptor.name.clone(),
        texture,
        view,
    })
}

fn create_buffer_object(
    device: &wgpu::Device,
    descriptor: &MagpieBackendBufferDescriptor,
) -> UpscaleResult<MagpieWgpuBufferObject> {
    if descriptor.byte_len == 0 {
        return Err(invalid_pipeline(format!(
            "Magpie buffer {} has zero byte length",
            descriptor.name
        )));
    }
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        mapped_at_creation: true,
        ..wgpu_buffer_descriptor(descriptor)
    });
    write_initial_dwords(&buffer, descriptor.byte_len, &descriptor.initial_dwords);
    buffer.unmap();

    Ok(MagpieWgpuBufferObject {
        name: descriptor.name.clone(),
        byte_len: descriptor.byte_len,
        buffer,
    })
}

fn create_sampler_object(
    device: &wgpu::Device,
    descriptor: &MagpieBackendSamplerDescriptor,
) -> MagpieWgpuSamplerObject {
    MagpieWgpuSamplerObject {
        name: descriptor.name.clone(),
        sampler: device.create_sampler(&wgpu_sampler_descriptor(descriptor)),
    }
}

fn write_initial_dwords(buffer: &wgpu::Buffer, byte_len: usize, dwords: &[u32]) {
    let mut view = buffer.slice(..).get_mapped_range_mut();
    let byte_count = byte_len.min(view.len());
    let mut bytes = vec![0_u8; byte_count];
    bytes
        .chunks_exact_mut(std::mem::size_of::<u32>())
        .zip(dwords.iter())
        .for_each(|(chunk, dword)| chunk.copy_from_slice(&dword.to_ne_bytes()));
    view.slice(..byte_count).copy_from_slice(&bytes);
    drop(view);
}

fn validate_wgsl_pass(
    layouts: &MagpieWgpuBindingLayoutPlan,
    pass: &MagpieWgpuShaderPass,
) -> UpscaleResult<()> {
    let layout = layouts.pass(pass.pass_index).ok_or_else(|| {
        invalid_pipeline(format!(
            "missing wgpu binding layout for pass {}",
            pass.pass_index
        ))
    })?;
    if layout.pass_name != pass.pass_name {
        return Err(invalid_pipeline(format!(
            "wgpu shader pass {} name {} does not match layout {}",
            pass.pass_index, pass.pass_name, layout.pass_name
        )));
    }
    if pass.entry_point.trim().is_empty() {
        return Err(invalid_pipeline(format!(
            "wgpu shader pass {} has an empty entry point",
            pass.pass_index
        )));
    }
    if pass.wgsl_source.trim().is_empty() {
        return Err(invalid_pipeline(format!(
            "wgpu shader pass {} has empty WGSL source",
            pass.pass_index
        )));
    }
    Ok(())
}

fn layout_object_by_pass<'a>(
    layouts: &'a [MagpieWgpuPassLayoutObjects],
    pass_index: u32,
    pass_name: &str,
) -> UpscaleResult<&'a MagpieWgpuPassLayoutObjects> {
    let layout = layouts
        .iter()
        .find(|layout| layout.pass_index == pass_index)
        .ok_or_else(|| {
            invalid_pipeline(format!("missing wgpu layout object for pass {pass_index}"))
        })?;
    if layout.pass_name != pass_name {
        return Err(invalid_pipeline(format!(
            "wgpu layout object pass {pass_index} name {} does not match shader {pass_name}",
            layout.pass_name
        )));
    }
    Ok(layout)
}

fn pipeline_by_pass<'a>(
    pipelines: &'a [MagpieWgpuPipelineObjects],
    pass_index: u32,
    pass_name: &str,
) -> UpscaleResult<&'a MagpieWgpuPipelineObjects> {
    let pipeline = pipelines
        .iter()
        .find(|pipeline| pipeline.pass_index == pass_index)
        .ok_or_else(|| {
            invalid_pipeline(format!(
                "missing wgpu compute pipeline for pass {pass_index}"
            ))
        })?;
    if pipeline.pass_name != pass_name {
        return Err(invalid_pipeline(format!(
            "wgpu compute pipeline pass {pass_index} name {} does not match execution pass {pass_name}",
            pipeline.pass_name
        )));
    }
    Ok(pipeline)
}

fn bind_group_by_pass<'a>(
    bind_groups: &'a [MagpieWgpuPassBindGroup],
    pass_index: u32,
    pass_name: &str,
) -> UpscaleResult<&'a MagpieWgpuPassBindGroup> {
    let bind_group = bind_groups
        .iter()
        .find(|bind_group| bind_group.pass_index == pass_index)
        .ok_or_else(|| {
            invalid_pipeline(format!("missing wgpu bind group for pass {pass_index}"))
        })?;
    if bind_group.pass_name != pass_name {
        return Err(invalid_pipeline(format!(
            "wgpu bind group pass {pass_index} name {} does not match execution pass {pass_name}",
            bind_group.pass_name
        )));
    }
    Ok(bind_group)
}

fn wgsl_declaration_pass(
    resources: &MagpieResourcePlan,
    pass: &MagpieResourcePass,
    layouts: &MagpieWgpuBindingLayoutPlan,
) -> UpscaleResult<MagpieWgpuDeclarationPass> {
    let layout = layouts.pass(pass.pass_index).ok_or_else(|| {
        invalid_pipeline(format!(
            "missing wgpu declaration layout for pass {}",
            pass.pass_index
        ))
    })?;
    let declarations = layout
        .bindings
        .iter()
        .map(|binding| wgsl_declaration(resources, pass, binding))
        .collect::<UpscaleResult<Vec<_>>>()?;
    let source = declarations
        .iter()
        .map(|declaration| declaration.source.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    Ok(MagpieWgpuDeclarationPass {
        pass_index: pass.pass_index,
        pass_name: pass.pass_name.clone(),
        declarations,
        source,
    })
}

fn wgsl_declaration(
    resources: &MagpieResourcePlan,
    pass: &MagpieResourcePass,
    binding: &MagpieWgpuBinding,
) -> UpscaleResult<MagpieWgpuDeclaration> {
    let source = match binding.kind {
        MagpieWgpuBindingKind::ConstantBuffer => {
            let buffer = constant_buffer_by_name(resources, &binding.name)?;
            wgsl_constant_buffer_declaration(binding, buffer)?
        }
        MagpieWgpuBindingKind::ShaderResource => {
            let texture = shader_resource_by_name(pass, &binding.name)?;
            wgsl_shader_resource_declaration(binding, texture)?
        }
        MagpieWgpuBindingKind::UnorderedAccess => {
            let texture = unordered_access_by_name(pass, &binding.name)?;
            wgsl_unordered_access_declaration(binding, texture)?
        }
        MagpieWgpuBindingKind::Sampler => wgsl_sampler_declaration(binding)?,
    };

    Ok(MagpieWgpuDeclaration {
        name: binding.name.clone(),
        binding: binding.binding,
        kind: binding.kind,
        source,
    })
}

fn wgsl_constant_buffer_declaration(
    binding: &MagpieWgpuBinding,
    buffer: &MagpieConstantBufferBinding,
) -> UpscaleResult<String> {
    validate_wgsl_identifier(&binding.name)?;
    let dword_count = buffer.byte_len.div_ceil(std::mem::size_of::<u32>());
    let type_name = format!("{}Data", binding.name);
    validate_wgsl_identifier(&type_name)?;
    Ok(format!(
        "struct {type_name} {{ data: array<u32, {dword_count}> }};\n@group(0) @binding({}) var<uniform> {}: {type_name};",
        binding.binding, binding.name
    ))
}

fn wgsl_shader_resource_declaration(
    binding: &MagpieWgpuBinding,
    texture: &MagpieTextureResourceBinding,
) -> UpscaleResult<String> {
    validate_wgsl_identifier(&binding.name)?;
    Ok(format!(
        "@group(0) @binding({}) var {}: texture_2d<{}>;",
        binding.binding,
        binding.name,
        wgsl_texture_sample_type(&texture.format)
    ))
}

fn wgsl_unordered_access_declaration(
    binding: &MagpieWgpuBinding,
    texture: &MagpieTextureResourceBinding,
) -> UpscaleResult<String> {
    validate_wgsl_identifier(&binding.name)?;
    Ok(format!(
        "@group(0) @binding({}) var {}: texture_storage_2d<{}, write>;",
        binding.binding,
        binding.name,
        wgsl_storage_texture_format(&texture.format)?
    ))
}

fn wgsl_sampler_declaration(binding: &MagpieWgpuBinding) -> UpscaleResult<String> {
    validate_wgsl_identifier(&binding.name)?;
    Ok(format!(
        "@group(0) @binding({}) var {}: sampler;",
        binding.binding, binding.name
    ))
}

fn wgsl_texture_sample_type(format: &MagpieTextureFormat) -> &'static str {
    match format.descriptor().component {
        MagpieTextureComponent::Float
        | MagpieTextureComponent::Unorm
        | MagpieTextureComponent::Snorm
        | MagpieTextureComponent::Unknown => "f32",
    }
}

fn wgsl_storage_texture_format(format: &MagpieTextureFormat) -> UpscaleResult<&'static str> {
    match format {
        MagpieTextureFormat::R8Unorm => Ok("r8unorm"),
        MagpieTextureFormat::R8g8b8a8Unorm | MagpieTextureFormat::R8g8b8a8UnormSrgb => {
            Ok("rgba8unorm")
        }
        MagpieTextureFormat::R8g8b8a8Snorm => Ok("rgba8snorm"),
        MagpieTextureFormat::R16g16b16a16Float => Ok("rgba16float"),
        MagpieTextureFormat::R32Float => Ok("r32float"),
        MagpieTextureFormat::R32g32b32a32Float => Ok("rgba32float"),
        MagpieTextureFormat::R8g8Unorm
        | MagpieTextureFormat::R16Float
        | MagpieTextureFormat::R16g16Float
        | MagpieTextureFormat::Dxgi(_)
        | MagpieTextureFormat::Unknown(_) => Err(invalid_pipeline(format!(
            "Magpie texture format {format:?} has no WGSL storage texture declaration"
        ))),
    }
}

fn validate_wgsl_identifier(identifier: &str) -> UpscaleResult<()> {
    let mut chars = identifier.chars();
    let Some(first) = chars.next() else {
        return Err(invalid_pipeline("WGSL identifier cannot be empty"));
    };
    let valid_first = first == '_' || first.is_ascii_alphabetic();
    let valid_rest = chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric());
    (valid_first && valid_rest)
        .then_some(())
        .ok_or_else(|| invalid_pipeline(format!("invalid WGSL identifier {identifier}")))
}

fn translate_simple_compute_pass(
    package: &MagpieEffectPackage,
    pass: &MagpieResourcePass,
    declarations: &MagpieWgpuDeclarationPlan,
) -> UpscaleResult<MagpieWgpuShaderPass> {
    let effect_pass_index = usize::try_from(pass.pass_index.saturating_sub(1))
        .map_err(|_| invalid_pipeline("invalid Magpie pass index"))?;
    let effect_pass = package
        .effect
        .passes
        .get(effect_pass_index)
        .ok_or_else(|| invalid_pipeline(format!("missing Magpie pass {}", pass.pass_index)))?;
    if effect_pass.style != MagpieFxPassStyle::Compute {
        return Err(invalid_pipeline(format!(
            "wgpu body translator currently supports only compute-style pass {}",
            pass.pass_index
        )));
    }
    if pass.shader_resources.len() != 1 || pass.unordered_access_views.len() != 1 {
        return Err(invalid_pipeline(format!(
            "wgpu simple copy translator requires exactly one input and one output for pass {}",
            pass.pass_index
        )));
    }
    let hlsl_source = package
        .compiler_pass_sources
        .get(effect_pass_index)
        .ok_or_else(|| invalid_pipeline(format!("missing pass source {}", pass.pass_index)))?;
    let input = &pass.shader_resources[0].name;
    let output = &pass.unordered_access_views[0].name;
    validate_simple_copy_source(hlsl_source, input, output)?;
    let declaration_source = declarations.pass(pass.pass_index).ok_or_else(|| {
        invalid_pipeline(format!(
            "missing WGSL declarations for pass {}",
            pass.pass_index
        ))
    })?;
    let wgsl_source = simple_copy_wgsl_source(declaration_source, pass, input, output)?;

    Ok(MagpieWgpuShaderPass::new(
        pass.pass_index,
        pass.pass_name.clone(),
        "main",
        wgsl_source,
    ))
}

fn validate_simple_copy_source(source: &str, input: &str, output: &str) -> UpscaleResult<()> {
    let normalized = source
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    let patterns = [
        format!("{output}[pos]={input}[pos];"),
        format!("{output}[pos.xy]={input}[pos.xy];"),
    ];
    patterns
        .iter()
        .any(|pattern| normalized.contains(pattern))
        .then_some(())
        .ok_or_else(|| {
            invalid_pipeline(format!(
                "wgpu simple copy translator could not match {output}[pos] = {input}[pos]"
            ))
        })
}

fn simple_copy_wgsl_source(
    declarations: &MagpieWgpuDeclarationPass,
    pass: &MagpieResourcePass,
    input: &str,
    output: &str,
) -> UpscaleResult<String> {
    validate_wgsl_identifier(input)?;
    validate_wgsl_identifier(output)?;
    let [workgroup_x, workgroup_y] = pass.dispatch.block_size;
    if workgroup_x == 0 || workgroup_y == 0 {
        return Err(invalid_pipeline(format!(
            "invalid WGSL workgroup size for pass {}",
            pass.pass_index
        )));
    }
    Ok(format!(
        "{declarations}\n\n@compute @workgroup_size({workgroup_x}, {workgroup_y}, 1)\nfn main(@builtin(global_invocation_id) gid: vec3<u32>) {{\n    let pos = vec2<u32>(gid.xy);\n    let input_size = textureDimensions({input});\n    let output_size = textureDimensions({output});\n    if (pos.x >= input_size.x || pos.y >= input_size.y || pos.x >= output_size.x || pos.y >= output_size.y) {{\n        return;\n    }}\n    let value = textureLoad({input}, vec2<i32>(pos), 0);\n    textureStore({output}, vec2<i32>(pos), value);\n}}\n",
        declarations = declarations.source
    ))
}

fn wgpu_texture_format(format: &MagpieTextureFormat) -> UpscaleResult<wgpu::TextureFormat> {
    match format {
        MagpieTextureFormat::R8Unorm => Ok(wgpu::TextureFormat::R8Unorm),
        MagpieTextureFormat::R8g8Unorm => Ok(wgpu::TextureFormat::Rg8Unorm),
        MagpieTextureFormat::R8g8b8a8Unorm => Ok(wgpu::TextureFormat::Rgba8Unorm),
        MagpieTextureFormat::R8g8b8a8UnormSrgb => Ok(wgpu::TextureFormat::Rgba8UnormSrgb),
        MagpieTextureFormat::R8g8b8a8Snorm => Ok(wgpu::TextureFormat::Rgba8Snorm),
        MagpieTextureFormat::R16Float => Ok(wgpu::TextureFormat::R16Float),
        MagpieTextureFormat::R16g16Float => Ok(wgpu::TextureFormat::Rg16Float),
        MagpieTextureFormat::R16g16b16a16Float => Ok(wgpu::TextureFormat::Rgba16Float),
        MagpieTextureFormat::R32Float => Ok(wgpu::TextureFormat::R32Float),
        MagpieTextureFormat::R32g32b32a32Float => Ok(wgpu::TextureFormat::Rgba32Float),
        MagpieTextureFormat::Dxgi(value) => Err(invalid_pipeline(format!(
            "DXGI format {value} has no wgpu mapping yet"
        ))),
        MagpieTextureFormat::Unknown(value) => Err(invalid_pipeline(format!(
            "unknown Magpie texture format {value} has no wgpu mapping"
        ))),
    }
}

fn wgpu_pass_layout(
    resources: &MagpieResourcePlan,
    pass: &MagpieResourcePass,
) -> UpscaleResult<MagpieWgpuPassLayout> {
    let constant_buffers = std::iter::once(&resources.constant_buffer)
        .chain(resources.dynamic_constant_buffer.iter())
        .map(wgpu_constant_buffer_binding);
    let shader_resources = pass
        .shader_resources
        .iter()
        .map(wgpu_shader_resource_binding);
    let unordered_access_views = pass
        .unordered_access_views
        .iter()
        .map(wgpu_unordered_access_binding);
    let samplers = pass.samplers.iter().map(wgpu_sampler_binding);
    let bindings = constant_buffers
        .chain(shader_resources)
        .chain(unordered_access_views)
        .chain(samplers)
        .collect::<UpscaleResult<Vec<_>>>()?;
    let entries = bindings
        .iter()
        .map(|binding| wgpu_bind_group_layout_entry(resources, pass, binding))
        .collect::<UpscaleResult<Vec<_>>>()?;

    Ok(MagpieWgpuPassLayout {
        pass_index: pass.pass_index,
        pass_name: pass.pass_name.clone(),
        bindings,
        entries,
    })
}

fn wgpu_constant_buffer_binding(
    binding: &MagpieConstantBufferBinding,
) -> UpscaleResult<MagpieWgpuBinding> {
    Ok(MagpieWgpuBinding {
        name: binding.name.clone(),
        register: binding.register,
        binding: binding_index(MAGPIE_WGPU_CONSTANT_BINDING_BASE, binding.register)?,
        kind: MagpieWgpuBindingKind::ConstantBuffer,
    })
}

fn wgpu_shader_resource_binding(
    binding: &MagpieTextureResourceBinding,
) -> UpscaleResult<MagpieWgpuBinding> {
    Ok(MagpieWgpuBinding {
        name: binding.name.clone(),
        register: binding.register,
        binding: binding_index(MAGPIE_WGPU_SHADER_RESOURCE_BINDING_BASE, binding.register)?,
        kind: MagpieWgpuBindingKind::ShaderResource,
    })
}

fn wgpu_unordered_access_binding(
    binding: &MagpieTextureResourceBinding,
) -> UpscaleResult<MagpieWgpuBinding> {
    Ok(MagpieWgpuBinding {
        name: binding.name.clone(),
        register: binding.register,
        binding: binding_index(MAGPIE_WGPU_UNORDERED_ACCESS_BINDING_BASE, binding.register)?,
        kind: MagpieWgpuBindingKind::UnorderedAccess,
    })
}

fn wgpu_sampler_binding(
    binding: &MagpieSamplerResourceBinding,
) -> UpscaleResult<MagpieWgpuBinding> {
    Ok(MagpieWgpuBinding {
        name: binding.name.clone(),
        register: binding.register,
        binding: binding_index(MAGPIE_WGPU_SAMPLER_BINDING_BASE, binding.register)?,
        kind: MagpieWgpuBindingKind::Sampler,
    })
}

fn binding_index(base: u32, register: u32) -> UpscaleResult<u32> {
    base.checked_add(register)
        .ok_or_else(|| invalid_pipeline("Magpie wgpu binding index overflowed"))
}

fn wgpu_bind_group_layout_entry(
    resources: &MagpieResourcePlan,
    pass: &MagpieResourcePass,
    binding: &MagpieWgpuBinding,
) -> UpscaleResult<wgpu::BindGroupLayoutEntry> {
    Ok(wgpu::BindGroupLayoutEntry {
        binding: binding.binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu_binding_type(resources, pass, binding)?,
        count: None,
    })
}

fn wgpu_binding_type(
    resources: &MagpieResourcePlan,
    pass: &MagpieResourcePass,
    binding: &MagpieWgpuBinding,
) -> UpscaleResult<wgpu::BindingType> {
    match binding.kind {
        MagpieWgpuBindingKind::ConstantBuffer => {
            let buffer = constant_buffer_by_name(resources, &binding.name)?;
            Ok(wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(buffer.byte_len as u64),
            })
        }
        MagpieWgpuBindingKind::ShaderResource => {
            let texture = shader_resource_by_name(pass, &binding.name)?;
            Ok(wgpu::BindingType::Texture {
                sample_type: wgpu_texture_sample_type(&texture.format),
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            })
        }
        MagpieWgpuBindingKind::UnorderedAccess => {
            let texture = unordered_access_by_name(pass, &binding.name)?;
            Ok(wgpu::BindingType::StorageTexture {
                access: wgpu::StorageTextureAccess::WriteOnly,
                format: wgpu_texture_format(&texture.format)?,
                view_dimension: wgpu::TextureViewDimension::D2,
            })
        }
        MagpieWgpuBindingKind::Sampler => {
            let sampler = sampler_by_name(pass, &binding.name)?;
            Ok(wgpu::BindingType::Sampler(wgpu_sampler_binding_type(
                sampler.filter,
            )))
        }
    }
}

fn constant_buffer_by_name<'a>(
    resources: &'a MagpieResourcePlan,
    name: &str,
) -> UpscaleResult<&'a MagpieConstantBufferBinding> {
    std::iter::once(&resources.constant_buffer)
        .chain(resources.dynamic_constant_buffer.iter())
        .find(|buffer| buffer.name == name)
        .ok_or_else(|| invalid_pipeline(format!("missing Magpie constant buffer {name}")))
}

fn shader_resource_by_name<'a>(
    pass: &'a MagpieResourcePass,
    name: &str,
) -> UpscaleResult<&'a MagpieTextureResourceBinding> {
    pass.shader_resources
        .iter()
        .find(|texture| texture.name == name)
        .ok_or_else(|| invalid_pipeline(format!("missing Magpie shader resource {name}")))
}

fn unordered_access_by_name<'a>(
    pass: &'a MagpieResourcePass,
    name: &str,
) -> UpscaleResult<&'a MagpieTextureResourceBinding> {
    pass.unordered_access_views
        .iter()
        .find(|texture| texture.name == name)
        .ok_or_else(|| invalid_pipeline(format!("missing Magpie unordered access view {name}")))
}

fn sampler_by_name<'a>(
    pass: &'a MagpieResourcePass,
    name: &str,
) -> UpscaleResult<&'a MagpieSamplerResourceBinding> {
    pass.samplers
        .iter()
        .find(|sampler| sampler.name == name)
        .ok_or_else(|| invalid_pipeline(format!("missing Magpie sampler {name}")))
}

fn wgpu_texture_sample_type(format: &MagpieTextureFormat) -> wgpu::TextureSampleType {
    match format.descriptor().component {
        MagpieTextureComponent::Float
        | MagpieTextureComponent::Unorm
        | MagpieTextureComponent::Snorm
        | MagpieTextureComponent::Unknown => wgpu::TextureSampleType::Float {
            filterable: wgpu_filterable_texture(format),
        },
    }
}

fn wgpu_filterable_texture(format: &MagpieTextureFormat) -> bool {
    matches!(
        format,
        MagpieTextureFormat::R8Unorm
            | MagpieTextureFormat::R8g8Unorm
            | MagpieTextureFormat::R8g8b8a8Unorm
            | MagpieTextureFormat::R8g8b8a8UnormSrgb
            | MagpieTextureFormat::R8g8b8a8Snorm
            | MagpieTextureFormat::R16Float
            | MagpieTextureFormat::R16g16Float
            | MagpieTextureFormat::R16g16b16a16Float
    )
}

fn wgpu_sampler_binding_type(filter: MagpieFxSamplerFilter) -> wgpu::SamplerBindingType {
    match filter {
        MagpieFxSamplerFilter::Point => wgpu::SamplerBindingType::NonFiltering,
        MagpieFxSamplerFilter::Linear => wgpu::SamplerBindingType::Filtering,
    }
}

fn wgpu_texture_usages(bind_flags: &[MagpieBackendTextureBind]) -> wgpu::TextureUsages {
    bind_flags
        .iter()
        .fold(wgpu::TextureUsages::empty(), |usage, bind| match bind {
            MagpieBackendTextureBind::ExternalInput | MagpieBackendTextureBind::SourceUpload => {
                usage | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING
            }
            MagpieBackendTextureBind::ExternalOutput => {
                usage | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::STORAGE_BINDING
            }
            MagpieBackendTextureBind::ShaderResource => {
                usage | wgpu::TextureUsages::TEXTURE_BINDING
            }
            MagpieBackendTextureBind::UnorderedAccess => {
                usage | wgpu::TextureUsages::STORAGE_BINDING
            }
        })
}

fn wgpu_address_mode(address: MagpieFxSamplerAddress) -> wgpu::AddressMode {
    match address {
        MagpieFxSamplerAddress::Clamp => wgpu::AddressMode::ClampToEdge,
        MagpieFxSamplerAddress::Wrap => wgpu::AddressMode::Repeat,
    }
}

fn wgpu_filter_mode(filter: MagpieFxSamplerFilter) -> wgpu::FilterMode {
    match filter {
        MagpieFxSamplerFilter::Point => wgpu::FilterMode::Nearest,
        MagpieFxSamplerFilter::Linear => wgpu::FilterMode::Linear,
    }
}

fn invalid_pipeline(message: impl Into<String>) -> UpscaleError {
    UpscaleError::InvalidPipeline(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{
        MagpieBackendBufferDescriptor, MagpieBackendSamplerDescriptor, MagpieBackendTextureBind,
        MagpieBackendTextureDescriptor,
    };
    use crate::formats::MagpieTextureComponent;
    use crate::formats::MagpieTextureFormatDescriptor;
    use crate::magpiefx::{MagpieFxSamplerAddress, MagpieFxSamplerFilter};
    use crate::package::MagpieEffectPackage;
    use crate::plan::MagpieRenderPlan;
    use crate::plan::MagpieTextureFormat;
    use crate::resources::MagpieResourcePlan;
    use crate::upload::{MagpieSourceUpload, MagpieSourceUploadPlan};
    use borderless_upscale_core::FrameSize;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn maps_texture_buffer_and_sampler_descriptors_to_wgpu() {
        let texture_binding = texture_descriptor();
        let texture = wgpu_texture_descriptor(&texture_binding).unwrap();
        assert_eq!(texture.format, wgpu::TextureFormat::Rgba8Unorm);
        assert_eq!(texture.size.width, 640);
        assert!(texture.usage.contains(wgpu::TextureUsages::TEXTURE_BINDING));
        assert!(texture.usage.contains(wgpu::TextureUsages::STORAGE_BINDING));

        let buffer_binding = MagpieBackendBufferDescriptor {
            name: "__CB1".to_owned(),
            register: 0,
            byte_len: 64,
            initial_dwords: vec![0; 16],
        };
        let buffer = wgpu_buffer_descriptor(&buffer_binding);
        assert_eq!(buffer.size, 64);
        assert!(buffer.usage.contains(wgpu::BufferUsages::UNIFORM));

        let sampler_binding = MagpieBackendSamplerDescriptor {
            name: "POINT".to_owned(),
            register: 0,
            filter: MagpieFxSamplerFilter::Point,
            address: MagpieFxSamplerAddress::Wrap,
        };
        let sampler = wgpu_sampler_descriptor(&sampler_binding);
        assert_eq!(sampler.mag_filter, wgpu::FilterMode::Nearest);
        assert_eq!(sampler.address_mode_u, wgpu::AddressMode::Repeat);
    }

    #[test]
    fn rejects_unmapped_dxgi_format() {
        let mut texture = texture_descriptor();
        texture.format.format = MagpieTextureFormat::Dxgi(98);

        assert!(matches!(
            wgpu_texture_descriptor(&texture),
            Err(UpscaleError::InvalidPipeline(_))
        ));
    }

    #[test]
    fn maps_pass_resources_to_wgpu_bind_group_layout_entries() {
        let package = package(
            r"
//!MAGPIE EFFECT
//!VERSION 4
//!USE _DYNAMIC
//!TEXTURE
Texture2D INPUT;
//!TEXTURE
//!WIDTH INPUT_WIDTH
//!HEIGHT INPUT_HEIGHT
Texture2D tex1;
//!TEXTURE
Texture2D OUTPUT;
//!SAMPLER
//!FILTER LINEAR
//!ADDRESS CLAMP
SamplerState LINEAR;
//!PASS 1
//!STYLE PS
//!IN INPUT
//!OUT tex1
//!DESC Copy
MF4 Pass1(float2 pos) { return INPUT.Sample(LINEAR, pos); }
//!PASS 2
//!IN tex1
//!OUT OUTPUT
//!BLOCK_SIZE 8
//!NUM_THREADS 64
void Pass2(uint2 pos) { OUTPUT[pos] = tex1[pos]; }
",
        );
        let resources = MagpieResourcePlan::from_package(&package).unwrap();

        let plan = MagpieWgpuBindingLayoutPlan::from_resource_plan(&resources).unwrap();
        let pass = plan.pass(1).unwrap();

        assert_eq!(pass.pass_name, "Copy");
        assert_eq!(pass.bindings.len(), 5);
        assert!(pass.entries.iter().any(|entry| {
            entry.binding == MAGPIE_WGPU_CONSTANT_BINDING_BASE
                && matches!(
                    entry.ty,
                    wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        ..
                    }
                )
        }));
        assert!(pass.entries.iter().any(|entry| {
            entry.binding == MAGPIE_WGPU_SHADER_RESOURCE_BINDING_BASE
                && matches!(entry.ty, wgpu::BindingType::Texture { .. })
        }));
        assert!(pass.entries.iter().any(|entry| {
            entry.binding == MAGPIE_WGPU_UNORDERED_ACCESS_BINDING_BASE
                && matches!(entry.ty, wgpu::BindingType::StorageTexture { .. })
        }));
        assert!(pass.entries.iter().any(|entry| {
            entry.binding == MAGPIE_WGPU_SAMPLER_BINDING_BASE
                && matches!(
                    entry.ty,
                    wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)
                )
        }));

        let descriptor = pass.bind_group_layout_descriptor();
        assert_eq!(descriptor.label, Some("Copy"));
        assert_eq!(descriptor.entries.len(), pass.entries.len());
        assert_eq!(descriptor.entries[0].binding, pass.entries[0].binding);
    }

    #[test]
    fn maps_wgsl_passes_to_shader_module_descriptors() {
        let package = package(
            r"
//!MAGPIE EFFECT
//!VERSION 4
//!TEXTURE
Texture2D INPUT;
//!TEXTURE
Texture2D OUTPUT;
//!PASS 1
//!IN INPUT
//!OUT OUTPUT
//!BLOCK_SIZE 8
//!NUM_THREADS 64
void Pass1(uint2 pos) { OUTPUT[pos] = INPUT[pos]; }
",
        );
        let resources = MagpieResourcePlan::from_package(&package).unwrap();
        let layouts = MagpieWgpuBindingLayoutPlan::from_resource_plan(&resources).unwrap();
        let shader_pass = MagpieWgpuShaderPass::new(
            1,
            "Pass 1",
            "main",
            "@compute @workgroup_size(8, 8, 1) fn main() {}",
        );

        let shaders = MagpieWgpuShaderPlan::from_wgsl_passes(&layouts, vec![shader_pass]).unwrap();
        let descriptor = shaders.pass(1).unwrap().shader_module_descriptor();

        assert_eq!(descriptor.label, Some("Pass 1"));
        assert!(matches!(
            descriptor.source,
            wgpu::ShaderSource::Wgsl(source) if source.contains("@compute")
        ));
    }

    #[test]
    fn rejects_wgsl_pass_without_matching_layout() {
        let layouts = MagpieWgpuBindingLayoutPlan { passes: Vec::new() };
        let shader_pass = MagpieWgpuShaderPass::new(1, "Pass 1", "main", "fn main() {}");

        assert!(matches!(
            MagpieWgpuShaderPlan::from_wgsl_passes(&layouts, vec![shader_pass]),
            Err(UpscaleError::InvalidPipeline(_))
        ));
    }

    #[test]
    fn generates_wgsl_resource_declarations_from_pass_layout() {
        let package = package(
            r"
//!MAGPIE EFFECT
//!VERSION 4
//!USE _DYNAMIC
//!TEXTURE
Texture2D INPUT;
//!TEXTURE
//!WIDTH INPUT_WIDTH
//!HEIGHT INPUT_HEIGHT
Texture2D tex1;
//!TEXTURE
Texture2D OUTPUT;
//!SAMPLER
//!FILTER LINEAR
//!ADDRESS CLAMP
SamplerState LINEAR;
//!PASS 1
//!STYLE PS
//!IN INPUT
//!OUT tex1
//!DESC Copy
MF4 Pass1(float2 pos) { return INPUT.Sample(LINEAR, pos); }
//!PASS 2
//!IN tex1
//!OUT OUTPUT
//!BLOCK_SIZE 8
//!NUM_THREADS 64
void Pass2(uint2 pos) { OUTPUT[pos] = tex1[pos]; }
",
        );
        let resources = MagpieResourcePlan::from_package(&package).unwrap();

        let declarations = MagpieWgpuDeclarationPlan::from_resource_plan(&resources).unwrap();
        let pass = declarations.pass(1).unwrap();

        assert!(pass.source.contains("struct __CB1Data { data: array<u32,"));
        assert!(
            pass.source
                .contains("@group(0) @binding(0) var<uniform> __CB1: __CB1Data;")
        );
        assert!(
            pass.source
                .contains("@group(0) @binding(1) var<uniform> __CB2: __CB2Data;")
        );
        assert!(
            pass.source
                .contains("@group(0) @binding(32) var INPUT: texture_2d<f32>;")
        );
        assert!(
            pass.source.contains(
                "@group(0) @binding(64) var tex1: texture_storage_2d<rgba8unorm, write>;"
            )
        );
        assert!(
            pass.source
                .contains("@group(0) @binding(96) var LINEAR: sampler;")
        );
    }

    #[test]
    fn rejects_unsupported_wgsl_storage_texture_declaration_format() {
        let package = package(
            r"
//!MAGPIE EFFECT
//!VERSION 4
//!TEXTURE
Texture2D INPUT;
//!TEXTURE
//!FORMAT R16_FLOAT
Texture2D OUTPUT;
//!PASS 1
//!IN INPUT
//!OUT OUTPUT
//!BLOCK_SIZE 8
//!NUM_THREADS 64
void Pass1(uint2 pos) { OUTPUT[pos] = INPUT[pos]; }
",
        );
        let resources = MagpieResourcePlan::from_package(&package).unwrap();

        assert!(matches!(
            MagpieWgpuDeclarationPlan::from_resource_plan(&resources),
            Err(UpscaleError::InvalidPipeline(_))
        ));
    }

    #[test]
    fn translates_simple_compute_copy_pass_to_wgsl() {
        let package = package(
            r"
//!MAGPIE EFFECT
//!VERSION 4
//!TEXTURE
Texture2D INPUT;
//!TEXTURE
Texture2D OUTPUT;
//!PASS 1
//!IN INPUT
//!OUT OUTPUT
//!BLOCK_SIZE 8
//!NUM_THREADS 64
void Pass1(uint2 pos) { OUTPUT[pos] = INPUT[pos]; }
",
        );
        let resources = MagpieResourcePlan::from_package(&package).unwrap();

        let plan =
            MagpieWgpuBodyTranslationPlan::from_package_and_resource_plan(&package, &resources)
                .unwrap();
        let pass = plan.pass(1).unwrap();

        assert_eq!(pass.entry_point, "main");
        assert!(
            pass.wgsl_source
                .contains("@compute @workgroup_size(8, 8, 1)")
        );
        assert!(pass.wgsl_source.contains("textureLoad(INPUT"));
        assert!(pass.wgsl_source.contains("textureStore(OUTPUT"));
        assert!(pass.wgsl_source.contains("textureDimensions(INPUT)"));
        assert!(pass.wgsl_source.contains("textureDimensions(OUTPUT)"));
        assert!(
            pass.wgsl_source
                .contains("pos.x >= input_size.x || pos.y >= input_size.y")
        );
        assert!(
            pass.wgsl_source
                .contains("pos.x >= output_size.x || pos.y >= output_size.y")
        );
        assert!(
            pass.wgsl_source
                .contains("@group(0) @binding(32) var INPUT")
        );
        assert!(
            pass.wgsl_source
                .contains("@group(0) @binding(64) var OUTPUT")
        );
    }

    #[test]
    fn rejects_compute_body_that_is_not_simple_copy() {
        let package = package(
            r"
//!MAGPIE EFFECT
//!VERSION 4
//!TEXTURE
Texture2D INPUT;
//!TEXTURE
Texture2D OUTPUT;
//!PASS 1
//!IN INPUT
//!OUT OUTPUT
//!BLOCK_SIZE 8
//!NUM_THREADS 64
void Pass1(uint2 pos) { OUTPUT[pos] = MF4(1, 0, 0, 1); }
",
        );
        let resources = MagpieResourcePlan::from_package(&package).unwrap();

        assert!(matches!(
            MagpieWgpuBodyTranslationPlan::from_package_and_resource_plan(&package, &resources),
            Err(UpscaleError::InvalidPipeline(_))
        ));
    }

    #[test]
    fn builds_execution_plan_from_resource_dispatches() {
        let package = package(
            r"
//!MAGPIE EFFECT
//!VERSION 4
//!TEXTURE
Texture2D INPUT;
//!TEXTURE
//!WIDTH INPUT_WIDTH
//!HEIGHT INPUT_HEIGHT
Texture2D tex1;
//!TEXTURE
Texture2D OUTPUT;
//!SAMPLER
SamplerState LINEAR;
//!PASS 1
//!STYLE PS
//!IN INPUT
//!OUT tex1
//!DESC Prep
MF4 Pass1(float2 pos) { return INPUT.Sample(LINEAR, pos); }
//!PASS 2
//!IN tex1
//!OUT OUTPUT
//!BLOCK_SIZE 8
//!NUM_THREADS 64
void Pass2(uint2 pos) { OUTPUT[pos] = tex1[pos]; }
",
        );
        let resources = MagpieResourcePlan::from_package(&package).unwrap();

        let execution = MagpieWgpuExecutionPlan::from_resource_plan(&resources);

        assert_eq!(execution.passes.len(), 2);
        assert_eq!(execution.pass(1).unwrap().pass_name, "Prep");
        assert_eq!(execution.pass(1).unwrap().group_count, [20, 15, 1]);
        assert_eq!(execution.pass(2).unwrap().pass_name, "Pass 2");
        assert_eq!(execution.pass(2).unwrap().group_count, [80, 60, 1]);
    }

    #[test]
    fn prepares_wgpu_effect_plan_from_package() {
        let package = package(
            r"
//!MAGPIE EFFECT
//!VERSION 4
//!TEXTURE
Texture2D INPUT;
//!TEXTURE
Texture2D OUTPUT;
//!PASS 1
//!IN INPUT
//!OUT OUTPUT
//!BLOCK_SIZE 8
//!NUM_THREADS 64
void Pass1(uint2 pos) { OUTPUT[pos] = INPUT[pos]; }
",
        );

        let prepared = MagpieWgpuPreparedEffectPlan::from_package(&package).unwrap();
        let descriptors = prepared.descriptor_plan().unwrap();

        assert_eq!(prepared.resources.passes.len(), 1);
        assert_eq!(prepared.backend_descriptors.textures.len(), 2);
        assert!(prepared.source_uploads.uploads.is_empty());
        assert_eq!(prepared.binding_layouts.passes.len(), 1);
        assert_eq!(prepared.declarations.passes.len(), 1);
        assert_eq!(prepared.shaders.passes.len(), 1);
        assert_eq!(prepared.execution.passes.len(), 1);
        assert_eq!(descriptors.textures.len(), 2);
        assert_eq!(descriptors.buffers.len(), 1);
        assert!(
            prepared
                .shaders
                .pass(1)
                .unwrap()
                .wgsl_source
                .contains("@compute @workgroup_size(8, 8, 1)")
        );
        assert_eq!(prepared.execution.pass(1).unwrap().group_count, [80, 60, 1]);
    }

    #[test]
    fn maps_source_upload_plan_to_wgpu_copy_layout_and_payload() {
        let path = unique_temp_file("wgpu-source-upload.dds");
        let payload = [1_u8, 2, 3, 4, 5, 6, 7, 8];
        fs::write(&path, [b"HEAD".as_slice(), payload.as_slice()].concat()).unwrap();
        let upload = source_upload(path.clone(), 4, 8);
        let plan = MagpieSourceUploadPlan {
            uploads: vec![upload],
        };

        let wgpu_uploads = MagpieWgpuSourceUploadPlan::from_source_upload_plan(&plan).unwrap();
        let wgpu_upload = wgpu_uploads.upload("SOURCE").unwrap();

        assert_eq!(wgpu_upload.texture_name, "SOURCE");
        assert_eq!(wgpu_upload.data_offset, 4);
        assert_eq!(wgpu_upload.data_byte_len, 8);
        assert_eq!(wgpu_upload.layout.offset, 0);
        assert_eq!(wgpu_upload.layout.bytes_per_row, Some(8));
        assert_eq!(wgpu_upload.layout.rows_per_image, Some(1));
        assert_eq!(wgpu_upload.size.width, 2);
        assert_eq!(wgpu_upload.size.height, 1);
        assert_eq!(wgpu_upload.size.depth_or_array_layers, 1);
        assert_eq!(wgpu_upload.read_data().unwrap(), payload);

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn rejects_short_source_upload_payload() {
        let path = unique_temp_file("wgpu-source-upload-short.dds");
        fs::write(&path, b"HEAD1234").unwrap();
        let upload =
            MagpieWgpuSourceUpload::from_source_upload(&source_upload(path.clone(), 4, 8)).unwrap();

        assert!(matches!(
            upload.read_data(),
            Err(UpscaleError::InvalidPipeline(_))
        ));

        fs::remove_file(path).unwrap();
    }

    fn texture_descriptor() -> MagpieBackendTextureDescriptor {
        MagpieBackendTextureDescriptor {
            name: "OUTPUT".to_owned(),
            size: FrameSize::new(640, 480).unwrap(),
            format: MagpieTextureFormatDescriptor {
                format: MagpieTextureFormat::R8g8b8a8Unorm,
                dxgi_format: Some(28),
                bytes_per_pixel: Some(4),
                channels: Some(4),
                component: MagpieTextureComponent::Unorm,
                normalized: true,
                signed: false,
                srgb: false,
            },
            dxgi_format: 28,
            bind_flags: vec![
                MagpieBackendTextureBind::ShaderResource,
                MagpieBackendTextureBind::UnorderedAccess,
            ],
        }
    }

    fn package(source: &str) -> MagpieEffectPackage {
        let effect = crate::magpiefx::parse_magpiefx(source).unwrap();
        let input_size = FrameSize::new(320, 240).unwrap();
        let output_size = FrameSize::new(640, 480).unwrap();
        let render_plan = MagpieRenderPlan::from_effect(&effect, input_size, output_size).unwrap();
        MagpieEffectPackage {
            effect_path: PathBuf::from("test.hlsl"),
            compiler_source: effect.hlsl_source.clone(),
            compiler_prelude_source: effect.prelude_source.clone(),
            compiler_common_source: effect.common_source.clone(),
            compiler_pass_sources: effect
                .passes
                .iter()
                .map(|pass| pass.source.clone())
                .collect(),
            effect,
            render_plan,
            includes: Vec::new(),
            source_assets: Vec::new(),
        }
    }

    fn source_upload(path: PathBuf, data_offset: usize, byte_len: u64) -> MagpieSourceUpload {
        MagpieSourceUpload {
            texture_name: "SOURCE".to_owned(),
            path,
            size: FrameSize::new(2, 1).unwrap(),
            format: texture_descriptor().format,
            data_offset,
            row_pitch: 8,
            byte_len,
        }
    }

    fn unique_temp_file(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("borderless-magpie-{}-{name}", std::process::id()))
    }
}
