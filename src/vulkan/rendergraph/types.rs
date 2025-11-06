//! Translation of graph_types.hpp and parts of graph_util.hpp/cpp
//!
//! Defines core enums, bitflags, and the PassEncoder.

use crate::error::AppResult;
use crate::vulkan;
use ash::vk;
use bitflags::bitflags;
use std::collections::HashMap;
use std::sync::Arc;
use vulkan::resource::{BufferData, ImageData, ResourceManager};
use vulkan::util::{ComputeCommandBuffer, GraphicsCommandBuffer, TransferCommandBuffer};

bitflags! {
    /// Corresponds to `vklib::types::Access`
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    #[repr(transparent)]
    pub struct Access: u32 {
        const None = 0;
        const Read = 1 << 0;
        const Write = 1 << 1;
        const ReadWrite = Self::Read.bits() | Self::Write.bits();
    }
}

/// Corresponds to `vklib::types::TaskType`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskType {
    Graphics,
    Compute,
    Transfer,
}

/// Corresponds to `vklib::types::PassEncoder`
/// This is the command buffer wrapper passed into a task's execute closure.
pub struct PassEncoder<'a> {
    pub cmd: vk::CommandBuffer,
    pub device: &'a ash::Device,
    pub res_manager: &'a Arc<ResourceManager>,
    // Store guards to keep resource data alive for the duration of the pass
    pub(crate) buffers_guard: &'a std::sync::RwLockReadGuard<'a, Vec<BufferData>>,
    pub(crate) images_guard: &'a std::sync::RwLockReadGuard<'a, Vec<ImageData>>,
}

impl<'a> PassEncoder<'a> {
    pub fn get_graphics_pass(&self, info: &vk::RenderingInfo) -> GraphicsCommandBuffer {
        GraphicsCommandBuffer::new(self.device, self.cmd, info)
    }

    pub fn get_transfer_pass(&self) -> TransferCommandBuffer {
        TransferCommandBuffer::new(self.device, self.cmd)
    }

    pub fn get_compute_pass(&self) -> ComputeCommandBuffer {
        ComputeCommandBuffer::new(self.device, self.cmd)
    }
}

/// Corresponds to `vklib::types::ExecuteFunction`
pub type ExecuteFunction = Box<dyn FnOnce(&mut PassEncoder) -> AppResult<()> + 'static>;

/// Translation of `graph_util.cpp`
///
/// A map of stage -> [ReadAccess, WriteAccess]
type StageAccessMap = HashMap<vk::PipelineStageFlags2, [vk::AccessFlags2; 2]>;

/// Corresponds to `getAccessesList`
pub fn get_accesses_list(
    stage: vk::PipelineStageFlags2,
    access_mask: Access,
) -> Vec<vk::AccessFlags2> {
    // This `match` statement replaces the static HashMap.
    // It's compile-time, zero-cost, and safer.
    let possible = match stage {
        vk::PipelineStageFlags2::TRANSFER => [
            vk::AccessFlags2::TRANSFER_READ,
            vk::AccessFlags2::TRANSFER_WRITE,
        ],
        vk::PipelineStageFlags2::VERTEX_SHADER => [
            vk::AccessFlags2::SHADER_READ,
            vk::AccessFlags2::SHADER_WRITE,
        ],
        vk::PipelineStageFlags2::FRAGMENT_SHADER => [
            vk::AccessFlags2::SHADER_READ,
            vk::AccessFlags2::SHADER_WRITE,
        ],
        vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT => [
            vk::AccessFlags2::COLOR_ATTACHMENT_READ,
            vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
        ],
        vk::PipelineStageFlags2::COMPUTE_SHADER => [
            vk::AccessFlags2::SHADER_READ,
            vk::AccessFlags2::SHADER_WRITE,
        ],
        // Add other stages as needed...
        _ => [vk::AccessFlags2::NONE, vk::AccessFlags2::NONE], // Default case
    };

    let mut result = Vec::new();
    if access_mask.contains(Access::Read) && possible[0] != vk::AccessFlags2::NONE {
        result.push(possible[0]);
    }
    if access_mask.contains(Access::Write) && possible[1] != vk::AccessFlags2::NONE {
        result.push(possible[1]);
    }
    result
}

/// Corresponds to `getAccessFlags`
pub fn get_access_flags(stage: vk::PipelineStageFlags2, access_mask: Access) -> vk::AccessFlags2 {
    get_accesses_list(stage, access_mask)
        .iter()
        .fold(vk::AccessFlags2::NONE, |acc, &flag| acc | flag)
}
