use crate::{library::VkLibrary, window::{App, WinitAppRunner}};

mod bindless;
mod library;
mod queue;
mod resource;
mod util;
mod window;
mod slang_api;

struct MyVulkanApp;

impl App for MyVulkanApp {
    fn initialize(&mut self, library: &mut VkLibrary) {
        log::info!("Application Initialized!");
        // Setup pipelines, resources, etc. here.
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

    WinitAppRunner::new()?.run_app(
        1280,        // desired width
        720,         // desired height
        MyVulkanApp, // your application logic
    )
}
