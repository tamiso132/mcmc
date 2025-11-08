//! Translation of window.hpp/cpp
//!
//! This has been rewritten to use 'winit' instead of 'glfw' to be
//! compatible with the 'ash-bootstrap' crate, which expects
//! 'raw-window-handle's.

use ash::{Instance, vk};
use winit::{
    dpi::PhysicalSize,
    event::{Event, KeyEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopBuilder}, // Added ControlFlow
    keyboard::{Key, NamedKey},
    raw_window_handle::{HasDisplayHandle, HasWindowHandle},
    window::{Window, WindowBuilder},
};

use super::library::VkLibrary; // Import VkLibrary from the library module

// Define a trait for the user's application logic
// This is the 'user stuff' that gets injected.
pub trait App {
    /// Called once at the start of the main loop.
    fn new(library: &mut VkLibrary) -> Self;
    /// Called for every frame.
    fn update(&mut self, library: &mut VkLibrary, dt: f32);
    /// Called on a window resize event.
    fn resize(&mut self, library: &mut VkLibrary, width: u32, height: u32);
}

/// Corresponds to GlfwWindow, but now using Winit.
pub struct WinitWindow {
    // In winit, the EventLoop must be owned by the main thread,
    // so we can't store it here. We return it from `new()`.
    pub window: Window,
}

impl WinitWindow {
    /// Creates a new window. The EventLoop is created externally now.
    pub fn create(
        w: u32,
        h: u32,
        title: &str,
        event_loop: &EventLoop<()>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let window = WindowBuilder::new()
            .with_title(title)
            .with_inner_size(winit::dpi::LogicalSize::new(w, h))
            .build(event_loop)?;

        Ok(Self { window })
    }

    /// Corresponds to createSurface.
    /// Uses 'ash-window' to create the surface from raw handles.
    pub fn create_surface(
        &self,
        entry: &ash::Entry,
        instance: &Instance,
    ) -> Result<vk::SurfaceKHR, vk::Result> {
        unsafe {
            ash_window::create_surface(
                entry,
                instance,
                self.window.display_handle().unwrap().as_raw(),
                self.window.window_handle().unwrap().as_raw(),
                None,
            )
        }
    }

    pub fn width(&self) -> u32 {
        self.window.inner_size().width
    }

    pub fn height(&self) -> u32 {
        self.window.inner_size().height
    }

    /// Helper to get the required instance extensions
    pub fn get_required_instance_extensions(
        &self,
    ) -> Result<Vec<&'static std::ffi::CStr>, Box<dyn std::error::Error>> {
        let extensions = ash_window::enumerate_required_extensions(
            self.window.display_handle().unwrap().as_raw(),
        )?;

        let extensions = extensions
            .iter()
            .map(|&s| unsafe { std::ffi::CStr::from_ptr(s) })
            .collect();
        Ok(extensions)
    }
}

/// New struct to own and run the Winit EventLoop.
pub struct WinitAppRunner {
    event_loop: EventLoop<()>,
}

impl WinitAppRunner {
    /// Creates a new WinitAppRunner and its EventLoop.
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            // EventLoopBuilder is used instead of EventLoop::new()
            // for more control, though EventLoop::new() would also work.
            event_loop: EventLoopBuilder::new().build()?,
        })
    }

    /// Creates the Vulkan library and runs the application's main loop.
    pub fn run_app<MyApp: App + 'static>(
        mut self,
        width: u32,
        height: u32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // --- 1. Create the Window and Vulkan Library ---
        // The WinitWindow needs the EventLoop reference now.
        let window = WinitWindow::create(width, height, "Vulkan App (Rust)", &self.event_loop)?;
        let mut library = VkLibrary::new(window);

        // --- 2. Initialize the User Application ---
        let mut user_app = MyApp::new(&mut library);

        // --- 3. Run the Event Loop ---
        log::info!("Starting Winit Event Loop...");

        let mut last_frame_instant = std::time::Instant::now();

        self.event_loop.run(move |event, window_target| {
            window_target.set_control_flow(ControlFlow::Poll);

            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    log::info!("The close button was pressed; stopping.");
                    window_target.exit();
                }
                Event::WindowEvent {
                    event: WindowEvent::RedrawRequested,
                    ..
                } => {
                    // Calculate Delta Time
                    let now = std::time::Instant::now();
                    let dt = now.duration_since(last_frame_instant).as_secs_f32();
                    last_frame_instant = now;

                    // Call user update logic
                    user_app.update(&mut library, dt);
                }
                Event::AboutToWait => {
                    // RedrawRequested will only trigger once, unless we manually request it.
                    library.window.window.request_redraw();
                }
                Event::WindowEvent {
                    event: WindowEvent::Resized(PhysicalSize { width, height }),
                    ..
                } => {
                    // Handle window resize event
                    if width > 0 && height > 0 {
                        log::info!("Window resized to: {}x{}", width, height);
                        user_app.resize(&mut library, width, height);
                    }
                }
                _ => (),
            }
        })?;

        // Note: Cleanup happens automatically when 'library' drops
        // after the event loop exits.

        Ok(())
    }
}
