//! Translation of pipelinebuilder.hpp, pipelinemanager.hpp, and pipeline.cpp
//!
//! This module provides a Rust-based pipeline builder and a pipeline manager
//! that integrates with the existing `SlangCompiler` and `VkLibrary`.
//!
//! REFACTORED:
//! - Integrated custom `AppError` and `AppResult` types.
//! - Removed lifetimes (`'a`) from pipeline configs and manager.
//!   Pipeline `CreateInfo` structs are now built on-the-fly from
//!   owned data, allowing `PipelineManager` to be 'static.

use crate::error::AppResult; // <-- ADDED
use super::slang_api::SlangCompiler;
use super::util::vk_check;
use ash::{Device, vk};
use shader_slang as slang; // Keep the slang alias
use std::collections::HashMap;
use std::ffi::CString;
use std::fs;
use std::path::Path;
use std::sync::{Arc, RwLock};

// --- From pipelinebuilder.hpp ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderType {
    None,
    Compute,
    Vertex,
    Fragment,
}

impl ShaderType {
    /// Helper to convert to Vulkan shader stage flags
    fn to_vk_stage(self) -> vk::ShaderStageFlags {
        match self {
            ShaderType::Compute => vk::ShaderStageFlags::COMPUTE,
            ShaderType::Vertex => vk::ShaderStageFlags::VERTEX,
            ShaderType::Fragment => vk::ShaderStageFlags::FRAGMENT,
            ShaderType::None => vk::ShaderStageFlags::empty(),
        }
    }
}

/// Corresponds to `ShaderFileInfo`
#[derive(Debug, Clone)]
pub struct ShaderFileInfo {
    pub path: String,
    pub entry: String,
    pub ty: ShaderType,
}

/// Corresponds to `GraphicsPipelineConfig`
/// This version is 'static and owns its data.
#[derive(Debug, Clone, Default)]
pub struct GraphicsPipelineConfig {
    pub input_assembly_topology: vk::PrimitiveTopology,
    pub rasterization_polygon_mode: vk::PolygonMode,
    pub rasterization_cull_mode: vk::CullModeFlags,
    pub depth_stencil_enable: bool,
    pub depth_stencil_compare_op: vk::CompareOp,
    pub dynamic_states: Vec<vk::DynamicState>,
    // TODO: Add color blend attachment state
}

/// Corresponds to `ComputePipelineConfig`
#[derive(Debug, Clone, Default)]
pub struct ComputePipelineConfig {}

/// Corresponds to `GraphicsPipelineBuilder`
#[derive(Default)]
pub struct GraphicsPipelineBuilder {
    config: GraphicsPipelineConfig,
    shaders: Vec<ShaderFileInfo>,
}

impl GraphicsPipelineBuilder {
    pub fn new() -> Self {
        // Set defaults from C++ constructor
        let mut config = GraphicsPipelineConfig::default();
        config.input_assembly_topology = vk::PrimitiveTopology::TRIANGLE_LIST;
        config.rasterization_polygon_mode = vk::PolygonMode::FILL;
        config.rasterization_cull_mode = vk::CullModeFlags::NONE;
        // config.rasterization.front_face = vk::FrontFace::COUNTER_CLOCKWISE; // This is default
        // config.rasterization.line_width = 1.0; // This is default

        Self {
            config,
            shaders: Vec::new(),
        }
    }

    pub fn add_shader(mut self, path: &str, entry: &str, ty: ShaderType) -> Self {
        self.shaders.push(ShaderFileInfo {
            path: path.to_string(),
            entry: entry.to_string(),
            ty,
        });
        self
    }

    pub fn set_rasterization(mut self, mode: vk::PolygonMode, cull: vk::CullModeFlags) -> Self {
        self.config.rasterization_polygon_mode = mode;
        self.config.rasterization_cull_mode = cull;
        self
    }

    pub fn set_topology(mut self, topology: vk::PrimitiveTopology) -> Self {
        self.config.input_assembly_topology = topology;
        self
    }

    pub fn add_dynamic_state(mut self, state: vk::DynamicState) -> Self {
        self.config.dynamic_states.push(state);
        self
    }

    pub fn build(self) -> GraphicsPipelineBuildResult {
        GraphicsPipelineBuildResult {
            config: self.config,
            shader_files: self.shaders,
        }
    }
}

/// Corresponds to `ComputePipelineBuilder`
#[derive(Default)]
pub struct ComputePipelineBuilder {
    config: ComputePipelineConfig,
    shader: Option<ShaderFileInfo>,
}

impl ComputePipelineBuilder {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn set_shader(mut self, path: &str, entry: &str, ty: ShaderType) -> Self {
        self.shader = Some(ShaderFileInfo {
            path: path.to_string(),
            entry: entry.to_string(),
            ty,
        });
        self
    }

    pub fn build(self) -> ComputePipelineBuildResult {
        ComputePipelineBuildResult {
            config: self.config,
            shader_info: self.shader.expect("Compute pipeline shader must be set"),
        }
    }
}

// --- Build Result Structs (from C++ builder) ---

#[derive(Debug, Clone)]
pub struct GraphicsPipelineBuildResult {
    pub config: GraphicsPipelineConfig,
    pub shader_files: Vec<ShaderFileInfo>,
}

#[derive(Debug, Clone)]
pub struct ComputePipelineBuildResult {
    pub config: ComputePipelineConfig,
    pub shader_info: ShaderFileInfo,
}

/// Corresponds to `PipelineBuilderResult` variant
#[derive(Debug, Clone)]
pub enum PipelineBuilderResult {
    Graphics(GraphicsPipelineBuildResult),
    Compute(ComputePipelineBuildResult),
}

// --- From pipelinemanager.hpp ---

pub type PipelineHandle = u32;

/// Corresponds to `PipelineManager::PipelineInfo`
struct PipelineInfo {
    name: String,
    slang_modules: Vec<slang::Module>,
    config: PipelineConfigVariant,
    pipeline: vk::Pipeline,
    ty: PipelineType,
}

/// Corresponds to `PipelineConfigVariant`
#[derive(Debug, Clone)]
pub enum PipelineConfigVariant {
    Graphics(GraphicsPipelineConfig),
    Compute(ComputePipelineConfig),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineType {
    Compute,
    Graphics,
}

/// Corresponds to `PipelineManager`
pub struct PipelineManager {
    device: Arc<Device>,
    slang_compiler: SlangCompiler,
    shared_layout: vk::PipelineLayout,
    output_shader_dir: String,
    pipelines: RwLock<HashMap<String, PipelineInfo>>,
    pipeline_vec: RwLock<Vec<vk::Pipeline>>,
}

impl PipelineManager {
    /// Corresponds to `PipelineManager` constructor
    pub fn new(
        device: Arc<Device>,
        shared_layout: vk::PipelineLayout,
        shader_dirs: Vec<String>,
        output_dir: &str,
    ) -> Self {
        // Convert search paths to CStrings for the Slang API
        let search_path_cstrings: Vec<CString> = shader_dirs
            .iter()
            .map(|s| CString::new(s.as_str()).expect("Failed to create CString"))
            .collect();

        let slang_compiler = SlangCompiler::new(search_path_cstrings, output_dir);

        Self {
            device,
            slang_compiler,
            shared_layout,
            output_shader_dir: output_dir.to_string(),
            pipelines: RwLock::new(HashMap::new()),
            pipeline_vec: RwLock::new(Vec::new()),
        }
    }

    /// Corresponds to `add_pipeline`
    pub fn add_pipeline(
        &self,
        name: &str,
        build_result: PipelineBuilderResult,
    ) -> AppResult<PipelineHandle> {
        // <-- Use AppResult
        let mut pipelines = self.pipelines.write().unwrap();
        let mut pipeline_vec = self.pipeline_vec.write().unwrap();

        if pipelines.contains_key(name) {
            log::warn!("Pipeline with name '{}' already exists.", name);
            for (i, p) in pipeline_vec.iter().enumerate() {
                if pipelines
                    .get(name)
                    .map_or(false, |info| info.pipeline == *p)
                {
                    return Ok(i as PipelineHandle);
                }
            }
        }

        match build_result {
            PipelineBuilderResult::Graphics(config) => {
                let mut shader_infos = Vec::new();
                let mut slang_modules = Vec::new();

                for file in &config.shader_files {
                    let slang_module = self
                        .slang_compiler
                        .compile_shader(&file.path, &file.entry)?; // <-- Use ?
                    slang_modules.push(slang_module);

                    let shader_module = self.load_shader_from_spv(&file.path)?; // <-- Use ?

                    shader_infos.push(PipelineShaderInfo {
                        module: shader_module,
                        entry: CString::new(file.entry.as_str())?, // <-- Use ?
                        stage: file.ty.to_vk_stage(),
                    });
                }

                let pipeline =
                    self.create_graphics_pipeline_internal(&config.config, &shader_infos);

                for info in shader_infos {
                    unsafe { self.device.destroy_shader_module(info.module, None) };
                }

                let pipeline_info = PipelineInfo {
                    name: name.to_string(),
                    slang_modules,
                    config: PipelineConfigVariant::Graphics(config.config),
                    pipeline,
                    ty: PipelineType::Graphics,
                };

                let handle = pipeline_vec.len() as PipelineHandle;
                pipelines.insert(name.to_string(), pipeline_info);
                pipeline_vec.push(pipeline);
                Ok(handle)
            }
            PipelineBuilderResult::Compute(config) => {
                let file = &config.shader_info;

                let slang_module = self
                    .slang_compiler
                    .compile_shader(&file.path, &file.entry)?; // <-- Use ?

                let shader_module = self.load_shader_from_spv(&file.path)?; // <-- Use ?

                let shader_info = PipelineShaderInfo {
                    module: shader_module,
                    entry: CString::new(file.entry.as_str())?, // <-- Use ?
                    stage: file.ty.to_vk_stage(),
                };

                let pipeline = self.create_compute_pipeline_internal(&config.config, &shader_info);

                unsafe { self.device.destroy_shader_module(shader_module, None) };

                let pipeline_info = PipelineInfo {
                    name: name.to_string(),
                    slang_modules: vec![slang_module],
                    config: PipelineConfigVariant::Compute(config.config),
                    pipeline,
                    ty: PipelineType::Compute,
                };

                let handle = pipeline_vec.len() as PipelineHandle;
                pipelines.insert(name.to_string(), pipeline_info);
                pipeline_vec.push(pipeline);
                Ok(handle)
            }
        }
    }

    pub fn get_pipeline(&self, name: &str) -> Option<vk::Pipeline> {
        self.pipelines
            .read()
            .unwrap()
            .get(name)
            .map(|info| info.pipeline)
    }

    pub fn get_pipeline_by_handle(&self, handle: PipelineHandle) -> Option<vk::Pipeline> {
        self.pipeline_vec
            .read()
            .unwrap()
            .get(handle as usize)
            .copied()
    }

    /// Corresponds to C++ `load_shader`
    fn load_shader_from_spv(&self, shader_name: &str) -> AppResult<vk::ShaderModule> {
        // <-- Use AppResult
        let output_path = if self.output_shader_dir.is_empty() {
            format!("{}.spv", shader_name)
        } else {
            format!("{}/{}.spv", self.output_shader_dir, shader_name)
        };

        let path = Path::new(&output_path);
        if !path.exists() {
            return Err(format!("Shader file not found: {}", output_path).into()); // <-- Use .into()
        }

        let code = fs::read(&output_path)?; // <-- Use ?

        let (prefix, code_u32, suffix) = unsafe { code.align_to::<u32>() };
        if !prefix.is_empty() || !suffix.is_empty() {
            return Err(format!("Shader code in {} is not u32-aligned.", output_path).into()); // <-- Use .into()
        }

        let info = vk::ShaderModuleCreateInfo::default().code(code_u32);

        let module = vk_check!(unsafe { self.device.create_shader_module(&info, None) });
        Ok(module)
    }

    /// Corresponds to `create_graphics_pipeline_internal`
    fn create_graphics_pipeline_internal(
        &self,
        config: &GraphicsPipelineConfig,
        shaders: &[PipelineShaderInfo],
    ) -> vk::Pipeline {
        let shader_stages: Vec<vk::PipelineShaderStageCreateInfo> = shaders
            .iter()
            .map(|s| {
                vk::PipelineShaderStageCreateInfo::default()
                    .stage(s.stage)
                    .module(s.module)
                    .name(&s.entry)
            })
            .collect();

        let vertex_input = vk::PipelineVertexInputStateCreateInfo::default();

        let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(config.input_assembly_topology);

        let rasterization = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(config.rasterization_polygon_mode)
            .cull_mode(config.rasterization_cull_mode)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE) // Default
            .line_width(1.0);

        let color_blend_attachment = vk::PipelineColorBlendAttachmentState::default()
            .color_write_mask(vk::ColorComponentFlags::RGBA);

        let color_blend = vk::PipelineColorBlendStateCreateInfo::default()
            .attachments(std::slice::from_ref(&color_blend_attachment));

        let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(config.depth_stencil_enable)
            .depth_write_enable(config.depth_stencil_enable)
            .depth_compare_op(config.depth_stencil_compare_op);

        let dynamic_state_info =
            vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&config.dynamic_states);

        // Viewport and multisample are empty if dynamic state is used
        let viewport_empty = vk::PipelineViewportStateCreateInfo::default();
        let multisample_empty = vk::PipelineMultisampleStateCreateInfo::default();

        let mut info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&shader_stages)
            .vertex_input_state(&vertex_input)
            .input_assembly_state(&input_assembly)
            .rasterization_state(&rasterization)
            .color_blend_state(&color_blend)
            .depth_stencil_state(&depth_stencil)
            .viewport_state(&viewport_empty)
            .multisample_state(&multisample_empty)
            .dynamic_state(&dynamic_state_info)
            .layout(self.shared_layout);

        // This is a minimal setup for dynamic viewport/scissor
        let viewport_info = vk::PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1);

        let multisample_info = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);

        // If not using dynamic states, provide them
        if config.dynamic_states.is_empty() {
            info.p_viewport_state = &viewport_info;
            info.p_multisample_state = &multisample_info;
        }

        let pipeline = vk_check!(unsafe {
            self.device.create_graphics_pipelines(
                vk::PipelineCache::null(),
                std::slice::from_ref(&info),
                None,
            )
        })[0];

        pipeline
    }

    /// Corresponds to `create_compute_pipeline_internal`
    fn create_compute_pipeline_internal(
        &self,
        _config: &ComputePipelineConfig,
        shader: &PipelineShaderInfo,
    ) -> vk::Pipeline {
        let stage = vk::PipelineShaderStageCreateInfo::default()
            .stage(shader.stage)
            .module(shader.module)
            .name(&shader.entry);

        let info = vk::ComputePipelineCreateInfo::default()
            .layout(self.shared_layout)
            .stage(stage);

        let pipeline = vk_check!(unsafe {
            self.device.create_compute_pipelines(
                vk::PipelineCache::null(),
                std::slice::from_ref(&info),
                None,
            )
        })[0];

        pipeline
    }
}

impl Drop for PipelineManager {
    fn drop(&mut self) {
        let pipelines = self.pipelines.write().unwrap();
        for (_, info) in pipelines.iter() {
            unsafe {
                self.device.destroy_pipeline(info.pipeline, None);
            }
        }
    }
}

/// Internal helper struct (from C++)
struct PipelineShaderInfo {
    module: vk::ShaderModule,
    entry: CString, // Must be CString to keep pointer alive
    stage: vk::ShaderStageFlags,
}
