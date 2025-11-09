use ash::vk::{self, Format, Pipeline, PrimitiveTopology};

use crate::vulkan::{
    library::VkLibrary,
    pipeline::{
        GraphicsPipelineBuilder, PipelineBuilderResult, PipelineHandle, PipelineManager, ShaderType,
    },
    window::{App, WinitAppRunner},
};

mod error;
// in src/main.rs
mod plugin;
mod shader;
mod vulkan;

struct MyVulkanApp {
    pipeline_manager: PipelineManager,
    quad_pipeline: PipelineHandle,
}

impl App for MyVulkanApp {
    fn new(library: &mut VkLibrary) -> Self {
        log::info!("Application Initialized!");
        // Setup pipelines, resources, etc. here.
        let pipeline_manager = PipelineManager::new(
            library.device.clone(),
            library.resource_manager.get_layout(),
            vec!["shaders/".to_owned()],
            "shaders/output",
        );

        let quad_pipeline = GraphicsPipelineBuilder::new()
            .add_shader("shader_quad.slang", "vs_main", ShaderType::Vertex)
            .add_shader("shader_quad.slang", "fs_main", ShaderType::Fragment)
            .add_color_attachments(&[vk::Format::R8G8B8A8_UNORM])
            .add_depth_format(Format::D32_SFLOAT)
            .set_topology(PrimitiveTopology::TRIANGLE_FAN)
            .build();

        // let quad_pipeline = GraphicsPipelineBuilder::new()
        //     .add_shader("simple.slang", "computeMain", ShaderType::Compute)
        //     .set_topology(PrimitiveTopology::TRIANGLE_FAN)
        //     .build();

        let quad_pipeline = pipeline_manager
            .add_pipeline(
                "QuadPipeline",
                PipelineBuilderResult::Graphics(quad_pipeline),
            )
            .unwrap();

        Self {
            pipeline_manager,
            quad_pipeline,
        }
    }

    fn update(&mut self, library: &mut VkLibrary, dt: f32) {
        // Main render loop logic goes here
        // Example: library.draw_frame();
    }

    fn resize(&mut self, library: &mut VkLibrary, width: u32, height: u32) {
        // Recreate swapchain and dependent resources here
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Other initial setup (e.g., logger)

    WinitAppRunner::new()?.run_app::<MyVulkanApp>(
        1280, // desired width
        720,  // desired height
    )
}
