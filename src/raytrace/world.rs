//! Replaces plugin.hpp/cpp
//!
//! This module provides a high-level `RaytraceWorld` struct
//! that encapsulates the SVO `Tree64Builder` and handles
//! its construction and GPU buffer creation.

use std::sync::Arc;

use super::map::Tree64Builder;
use crate::vulkan::library::VkLibrary;
use crate::vulkan::resource::{ResourceHandle, ResourceManager};
use crate::vulkan::util::{self, TBufferInfo};
use glam::Vec3;
use vk_mem as vma;

/// This struct holds the raytracing acceleration structure.
/// It replaces the C++ `RaytraceW` singleton.
pub struct RaytraceWorld {
    pub tree: Tree64Builder,
    pub tree_buffer: Option<ResourceHandle>,
    // You will also need a buffer for the HashMap data.
    // pub hashmap_buffer: Option<ResourceHandle>,
}

impl RaytraceWorld {
    /// Creates a new world and builds the SVO.
    pub fn new() -> Self {
        // Run the bitstream test function on startup
        super::node::Bitstream64::test_function();

        // C++: ray_world.tree = Tree64Builder(SHADER_MAX_TREE_LEVEL, {0, 0, 0});
        // SHADER_MAX_TREE_LEVEL from `rayshared.inl` is assumed to be 4.
        const SHADER_MAX_TREE_LEVEL: u32 = 4;
        let mut tree = Tree64Builder::new(SHADER_MAX_TREE_LEVEL, Vec3::ZERO);

        // --- This block replaces loading "assets/castle.vox" ---
        // We use the test data from your plugin.cpp
        log::info!("Inserting test box into octree...");
        // C++: ray_world.tree.insert_box(glm::vec3(0, 0, 0), glm::vec3(4, 4, 4));
        tree.insert_box(Vec3::new(0.0, 0.0, 0.0), Vec3::new(4.0, 4.0, 4.0));
        // --- End test block ---

        tree.print_profile();
        tree.sort();
        tree.print_profile();

        log::info!("Octree built and sorted.");

        Self {
            tree,
            tree_buffer: None,
        }
    }

    /// Call this to create the GPU buffer for the tree.
    /// This should be called once during initialization.
    pub fn create_gpu_buffers(&mut self, resource_manager: &Arc<ResourceManager>) {
        let node_data_u64 = self.tree.get_node_data();

        // Need to convert &[u64] to &[u8] for buffer creation
        let node_data_u8: &[u8] = unsafe {
            std::slice::from_raw_parts(
                node_data_u64.as_ptr() as *const u8,
                node_data_u64.len() * std::mem::size_of::<u64>(),
            )
        };

        if node_data_u8.is_empty() {
            log::warn!("Octree node data is empty, skipping buffer creation.");
            return;
        }

        log::info!("Creating GPU octree buffer of {} bytes", node_data_u8.len());

        // Create the storage buffer
        let buffer_info = util::helper::storage_buffer_info(
            node_data_u8.len() as u64,
            vma::MemoryUsage::AutoPreferDevice, // Upload to GPU
        );

        // TODO: This buffer needs to be filled!
        // Your current ResourceManager::create_buffer only creates the buffer.
        // You will need to implement an upload (e.g., via a staging buffer)
        // to get `node_data_u8` into this new buffer.

        // For now, we just create the device-local buffer.
        let tree_buffer_handle =
            resource_manager.create_buffer("Octree Node Buffer".to_string(), buffer_info);

        self.tree_buffer = Some(tree_buffer_handle);

        // You would also create and upload the HashMap buffer here
        // (which is `self.tree.nodes.array_index_map`)
    }
}



