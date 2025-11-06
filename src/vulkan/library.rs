//! Translation of library.cpp
//!
//! This version manually initializes Vulkan using `ash`, `ash-window`,
//! and `winit`, replacing the `ash-bootstrap` crate.
//! This is a custom "bootstrap" process.

use super::bindless::BindlessDescriptors;
use super::queue::{QueueCapability, QueueHandle, QueueInfo, QueueManager}; // Import QueueManager
use super::resource::{ResourceHandle, ResourceManager};
use super::util::{helper, vk_check};
use super::window::{WinitWindow, WinitAppRunner, App}; // Import WinitWindow, WinitAppRunner, and App
use ash::{ext, khr, vk}; 
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::Arc;
use vk_mem as vma;

// Helper to convert Rust string slices to C-style strings
fn str_to_raw(s: &str) -> CString {
    CString::new(s).expect("Failed to create CString")
}

// Helper for debug messenger
unsafe extern "system" fn vulkan_debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _p_user_data: *mut std::ffi::c_void,
) -> vk::Bool32 {
    let message = CStr::from_ptr((*p_callback_data).p_message).to_string_lossy();
    match message_severity {
        vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => log::error!("[VK_ERROR] {}", message),
        vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => log::warn!("[VK_WARN] {}", message),
        vk::DebugUtilsMessageSeverityFlagsEXT::INFO => log::info!("[VK_INFO] {}", message),
        _ => log::trace!("[VK_VERBOSE] {}", message),
    }
    vk::FALSE
}

/// The main Vulkan context, holding all core objects.
/// Corresponds to C++ VkLibrary.
pub struct VkLibrary {
    // window is now public and owned
    pub window: WinitWindow, 
    pub entry: ash::Entry,
    pub instance: ash::Instance,
    pub device: Arc<ash::Device>, // Arc for sharing with ResourceManager
    pub physical_device: vk::PhysicalDevice,
    pub queue: vk::Queue,
    pub queue_family: u32,
    pub queue_manager: QueueManager, // <-- ADDED: Instance of QueueManager
    pub allocator: Arc<vma::Allocator>, // Arc for sharing
    pub resource_manager: Arc<ResourceManager>,
    // Surface members
    pub surface_loader: khr::surface::Instance,
    pub surface: vk::SurfaceKHR,
    // Swapchain members
    pub swapchain_loader: khr::swapchain::Device,
    pub swapchain: vk::SwapchainKHR,
    pub swapchain_images: Vec<ResourceHandle>,
    // Debug utilities must be stored to be valid
    debug_utils: Option<ext::debug_utils::Instance>,
    debug_messenger: Option<vk::DebugUtilsMessengerEXT>,
}

impl VkLibrary {
    /// Corresponds to `VkLibrary<T>::create`
    /// Now accepts an already created WinitWindow.
    pub fn new(window: WinitWindow) -> Self {
        let entry = vk_check!(unsafe { ash::Entry::load() });
        let initial_width = window.width();
        let initial_height = window.height();

        // --- 2. Create Instance ---
        let (instance, debug_utils, debug_messenger) =
            Self::create_instance(&entry, &window);

        // --- 3. Create Surface ---
        // Uses the new surface creation function on WinitWindow
        let surface_loader = khr::surface::Instance::new(&entry, &instance);
        let surface = vk_check!(window.create_surface(&entry, &instance));

        // --- 4. Select Physical Device ---
        let (physical_device, queue_family) =
            Self::select_physical_device(&instance, &surface_loader, surface);

        // --- 5. Create Logical Device & Queue ---
        let (device, queue) = Self::create_logical_device(&instance, physical_device, queue_family);
        let device = Arc::new(device);

        // --- 5b. Create QueueManager and register the queue ---
        let queue_manager = QueueManager::new();
        queue_manager.register_queue(queue, queue_family, QueueCapability::Graphics);

        // --- 6. Create VMA Allocator ---
        let allocator = {
            let vma_info = vma::AllocatorCreateInfo::new(&instance, &device, physical_device);
            unsafe { Arc::new(vk_check!(vma::Allocator::new(vma_info))) }
        };

        // --- 7. Create Resource Manager ---
        let resource_manager = Arc::new(ResourceManager::new(allocator.clone(), device.clone()));

        // --- 8. Create Swapchain ---
        let swapchain_loader = khr::swapchain::Device::new(&instance, &device);
        let (swapchain, swapchain_format, swapchain_extent) = Self::create_swapchain(
            &instance,
            &device,
            physical_device,
            &surface_loader,
            surface,
            &swapchain_loader,
            initial_width,
            initial_height,
        );

        // --- 9. Register Swapchain Images ---
        let swapchain_images = Self::register_swapchain_images(
            &device, 
            &swapchain_loader, 
            swapchain, 
            swapchain_format, 
            swapchain_extent, 
            &resource_manager
        );

        log::info!("Vulkan initialized successfully (manual ash setup)!");

        Self {
            window,
            entry,
            instance,
            device,
            physical_device,
            queue,
            queue_family,
            queue_manager,
            allocator,
            resource_manager,
            surface_loader,
            surface,
            swapchain_loader,
            swapchain,
            swapchain_images,
            debug_utils,
            debug_messenger,
        }
    }

    /// Extracted logic to register swapchain images
    fn register_swapchain_images(
        device: &ash::Device,
        swapchain_loader: &khr::swapchain::Device,
        swapchain: vk::SwapchainKHR,
        swapchain_format: vk::Format,
        swapchain_extent: vk::Extent2D,
        resource_manager: &Arc<ResourceManager>,
    ) -> Vec<ResourceHandle> {
        let images = vk_check!(unsafe { swapchain_loader.get_swapchain_images(swapchain) });
        let mut handles = Vec::with_capacity(images.len());

        let usage = vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_DST;
        let info = helper::sampled_image_info(
            swapchain_extent.width,
            swapchain_extent.height,
            swapchain_format,
            usage,
        );

        for &image in images.iter() {
            let mut image_info = info.clone();
            image_info.info.initial_layout = vk::ImageLayout::UNDEFINED;

            let view_info = vk::ImageViewCreateInfo {
                s_type: vk::StructureType::IMAGE_VIEW_CREATE_INFO,
                image,
                view_type: vk::ImageViewType::TYPE_2D,
                format: swapchain_format,
                components: vk::ComponentMapping::default(),
                subresource_range: vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                },
                ..Default::default()
            };

            let view = vk_check!(unsafe { device.create_image_view(&view_info, None) });
            let handle = resource_manager.register_image(image_info, image, view);
            handles.push(handle);
        }
        handles
    }
    
    // --- The rest of the helper functions remain in library.rs ---

    /// Helper 2: Create Instance
    fn create_instance(
        entry: &ash::Entry,
        window: &WinitWindow, // Now accepts WinitWindow
    ) -> (
        ash::Instance,
        Option<ext::debug_utils::Instance>, 
        Option<vk::DebugUtilsMessengerEXT>,
    ) {
        let app_name = str_to_raw("Vulkan App");
        let engine_name = str_to_raw("Custom Engine");

        let app_info = vk::ApplicationInfo {
            s_type: vk::StructureType::APPLICATION_INFO,
            p_application_name: app_name.as_ptr(),
            application_version: vk::make_api_version(0, 1, 0, 0),
            p_engine_name: engine_name.as_ptr(),
            engine_version: vk::make_api_version(0, 1, 0, 0),
            api_version: vk::API_VERSION_1_3,
            ..Default::default()
        };

        // Get required extensions from winit
        let mut required_extensions =
            window.get_required_instance_extensions() // Use WinitWindow method
                .expect("Failed to get required instance extensions")
                .into_iter()
                .map(|s| s.as_ptr())
                .collect::<Vec<*const c_char>>();

        let validation_layers = [str_to_raw("VK_LAYER_KHRONOS_validation")];
        let mut enable_validation = true;
        
        // Add debug extensions if validation is on
        if enable_validation {
            required_extensions.push(ext::debug_utils::NAME.as_ptr());
        }

        // Check if layers are available
        let layer_props = vk_check!(unsafe { entry.enumerate_instance_layer_properties() });
        for layer in &validation_layers {
            let found = layer_props.iter().any(|props| {
                let name = unsafe { CStr::from_ptr(props.layer_name.as_ptr()) };
                name == layer.as_c_str()
            });
            if !found {
                log::warn!("Validation layer not found: {:?}", layer);
                enable_validation = false;
                break;
            }
        }

        let validation_layers_raw: Vec<*const c_char> = if enable_validation {
            validation_layers.iter().map(|s| s.as_ptr()).collect()
        } else {
            Vec::new()
        };

        let mut create_info = vk::InstanceCreateInfo {
            s_type: vk::StructureType::INSTANCE_CREATE_INFO,
            p_application_info: &app_info,
            pp_enabled_extension_names: required_extensions.as_ptr(),
            enabled_extension_count: required_extensions.len() as u32,
            pp_enabled_layer_names: validation_layers_raw.as_ptr(),
            enabled_layer_count: validation_layers_raw.len() as u32,
            ..Default::default()
        };

        // Enable debug messenger for instance creation
        let mut debug_info = vk::DebugUtilsMessengerCreateInfoEXT {
            s_type: vk::StructureType::DEBUG_UTILS_MESSENGER_CREATE_INFO_EXT,
            message_severity: vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
            message_type: vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
            pfn_user_callback: Some(vulkan_debug_callback),
            ..Default::default()
        };

        if enable_validation {
            create_info.p_next = &debug_info as *const _ as *const std::ffi::c_void;
        }

        let instance = vk_check!(unsafe { entry.create_instance(&create_info, None) });

        // Load debug utils functions and create the messenger
        if enable_validation {
            let debug_utils = ext::debug_utils::Instance::new(entry, &instance);
            let debug_messenger =
                vk_check!(unsafe { debug_utils.create_debug_utils_messenger(&debug_info, None) });
            (instance, Some(debug_utils), Some(debug_messenger))
        } else {
            (instance, None, None)
        }
    }

    /// Helper 4: Select Physical Device
    fn select_physical_device(
        instance: &ash::Instance,
        surface_loader: &khr::surface::Instance, 
        surface: vk::SurfaceKHR,
    ) -> (vk::PhysicalDevice, u32) {
        let physical_devices = vk_check!(unsafe { instance.enumerate_physical_devices() });
        let (pdevice, qfamily) = physical_devices
            .into_iter()
            .find_map(|pdevice| {
                let queue_family = unsafe {
                    instance
                        .get_physical_device_queue_family_properties(pdevice)
                        .into_iter()
                        .enumerate()
                        .find_map(|(i, props)| {
                            let supports_graphics =
                                props.queue_flags.contains(vk::QueueFlags::GRAPHICS);
                            let supports_surface = vk_check!(unsafe {
                                surface_loader
                                    .get_physical_device_surface_support(pdevice, i as u32, surface)
                            });
                            if supports_graphics && supports_surface {
                                Some(i as u32)
                            } else {
                                None
                            }
                        })
                };
                queue_family.map(|qf| (pdevice, qf))
            })
            .expect("No suitable physical device found");

        let props = unsafe { instance.get_physical_device_properties(pdevice) };
        log::info!("Selected GPU: {:?}", unsafe {
            CStr::from_ptr(props.device_name.as_ptr())
        });
        (pdevice, qfamily)
    }

    /// Helper 5: Create Logical Device
    fn create_logical_device(
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        queue_family: u32,
    ) -> (ash::Device, vk::Queue) {
        let queue_priorities = [1.0f32];
        let queue_info = vk::DeviceQueueCreateInfo {
            s_type: vk::StructureType::DEVICE_QUEUE_CREATE_INFO,
            queue_family_index: queue_family,
            queue_count: 1,
            p_queue_priorities: queue_priorities.as_ptr(),
            ..Default::default()
        };

        let device_extensions = [khr::swapchain::NAME.as_ptr()];

        // Enable required 1.3 features
        let mut features_1_3 = vk::PhysicalDeviceVulkan13Features {
            s_type: vk::StructureType::PHYSICAL_DEVICE_VULKAN_1_3_FEATURES,
            dynamic_rendering: vk::TRUE,
            synchronization2: vk::TRUE,
            ..Default::default()
        };
        // Enable required 1.2 features
        let mut features_1_2 = vk::PhysicalDeviceVulkan12Features {
            s_type: vk::StructureType::PHYSICAL_DEVICE_VULKAN_1_2_FEATURES,
            buffer_device_address: vk::TRUE,
            ..Default::default()
        };

        let mut device_create_info = vk::DeviceCreateInfo {
            s_type: vk::StructureType::DEVICE_CREATE_INFO,
            p_queue_create_infos: &queue_info,
            queue_create_info_count: 1,
            pp_enabled_extension_names: device_extensions.as_ptr(),
            enabled_extension_count: device_extensions.len() as u32,
            p_next: &mut features_1_3 as *mut _ as *mut std::ffi::c_void,
            ..Default::default()
        };
        features_1_3.p_next = &mut features_1_2 as *mut _ as *mut std::ffi::c_void;

        let device = vk_check!(unsafe {
            instance.create_device(physical_device, &device_create_info, None)
        });
        let queue = unsafe { device.get_device_queue(queue_family, 0) };

        (device, queue)
    }

    /// Helper 8: Create Swapchain
    fn create_swapchain(
        instance: &ash::Instance,
        device: &ash::Device,
        physical_device: vk::PhysicalDevice,
        surface_loader: &khr::surface::Instance, 
        surface: vk::SurfaceKHR,
        swapchain_loader: &khr::swapchain::Device, 
        width: u32,
        height: u32,
    ) -> (vk::SwapchainKHR, vk::Format, vk::Extent2D) {
        // Get surface capabilities
        let caps = vk_check!(unsafe {
            surface_loader.get_physical_device_surface_capabilities(physical_device, surface)
        });
        let formats = vk_check!(unsafe {
            surface_loader.get_physical_device_surface_formats(physical_device, surface)
        });
        let present_modes = vk_check!(unsafe {
            surface_loader.get_physical_device_surface_present_modes(physical_device, surface)
        });

        // Select format
        let format = formats
            .iter()
            .find(|f| {
                f.format == vk::Format::B8G8R8A8_UNORM
                    && f.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
            })
            .unwrap_or(&formats[0]);

        // Select present mode
        let present_mode = present_modes
            .iter()
            .find(|p| **p == vk::PresentModeKHR::FIFO) // Try for FIFO (vsync)
            .unwrap_or(&present_modes[0]);

        // Select extent
        let extent = if caps.current_extent.width != u32::MAX {
            caps.current_extent
        } else {
            vk::Extent2D {
                width: width.clamp(caps.min_image_extent.width, caps.max_image_extent.width),
                height: height.clamp(caps.min_image_extent.height, caps.max_image_extent.height),
            }
        };

        // Select image count
        let mut image_count = caps.min_image_count + 1;
        if caps.max_image_count > 0 && image_count > caps.max_image_count {
            image_count = caps.max_image_count;
        }

        let swapchain_info = vk::SwapchainCreateInfoKHR {
            s_type: vk::StructureType::SWAPCHAIN_CREATE_INFO_KHR,
            surface,
            min_image_count: image_count,
            image_format: format.format,
            image_color_space: format.color_space,
            image_extent: extent,
            image_array_layers: 1,
            image_usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_DST,
            image_sharing_mode: vk::SharingMode::EXCLUSIVE,
            pre_transform: caps.current_transform,
            composite_alpha: vk::CompositeAlphaFlagsKHR::OPAQUE,
            present_mode: *present_mode,
            clipped: vk::TRUE,
            old_swapchain: vk::SwapchainKHR::null(),
            ..Default::default()
        };

        let swapchain =
            vk_check!(unsafe { swapchain_loader.create_swapchain(&swapchain_info, None) });

        (swapchain, format.format, extent)
    }

    /// Destructor
    pub fn cleanup(&mut self) {
        unsafe {
            // Wait for device to be idle
            vk_check!(self.device.device_wait_idle());

            // Destroy swapchain
            self.swapchain_loader
                .destroy_swapchain(self.swapchain, None);

            // Destroy device
            if let Some(device) = Arc::get_mut(&mut self.device) {
                device.destroy_device(None);
            } else {
                log::warn!(
                    "Could not get mutable access to Device for cleanup. It may still be in use."
                );
            }

            // Destroy surface
            self.surface_loader.destroy_surface(self.surface, None);

            // Destroy debug messenger
            if let (Some(utils), Some(messenger)) =
                (self.debug_utils.as_ref(), self.debug_messenger)
            {
                utils.destroy_debug_utils_messenger(messenger, None);
            }

            // Destroy instance
            self.instance.destroy_instance(None);
        }
        log::info!("Vulkan resources cleaned up.");
    }
}

// Implement Drop to automatically call cleanup
impl Drop for VkLibrary {
    fn drop(&mut self) {
        self.cleanup();
    }
}