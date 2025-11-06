//! Translation of util.hpp/cpp
//!
//! Contains error-handling macros, Vulkan info helpers,
//! and command buffer wrapper structs. Uses 'ash'.
//!
//! CORRECTED: Reverted to a panicking `vk_check!` macro as requested.
//! This macro now logs and panics directly on an `ash::vk::Result`
//! without using a custom  `VkError` enum.
//!
//! CORRECTED: Added lifetime 'a to TBufferInfo/TImageInfo and
//! implemented Debug manually to handle non-Debug VMA structs.
//!
//! CORRECTED: Replaced `.builder()` pattern with `::default()`
//! for broader ash version compatibility.

use ash::{Device, vk};
use std::fmt;
use std::marker::PhantomData;
use vk_mem as vma; // Use 'vma' as the alias, like in C++

// Custom VkError enum has been removed as requested.

/// Corresponds to the C++ VK_CHECK macros.
///
/// This macro now logs and panics if the expression
/// returns an `Err(vk::Result)`.
#[macro_export]
macro_rules! vk_check {
    ($call:expr) => {
        match $call {
            // If the call is Ok(value), return the value.
            Ok(val) => val,
            // If the call is Err(err), log it and panic.
            Err(err) => {
                // 'err' is the raw vk::Result from ash or vk-mem-rs.
                log::error!("Vulkan error at {}:{}: {:?}", file!(), line!(), err);
                // Panic just like the C++ abort().
                panic!("Vulkan error: {:?}", err);
            }
        }
    };
}
pub use vk_check; // Re-export the macro

// --- Corresponds to types in util.hpp ---

/// Wrapper for vk::BufferCreateInfo that includes VMA info.
/// Generic over lifetime 'a due to ash::vk::BufferCreateInfo<'a>.
#[derive(Clone)]
pub struct TBufferInfo<'a> {
    pub info: vk::BufferCreateInfo<'a>,
    pub alloc_info: vma::AllocationCreateInfo,
    _marker: PhantomData<&'a ()>,
}

/// Manual Debug impl as vma::AllocationCreateInfo doesn't impl Debug.
impl<'a> fmt::Debug for TBufferInfo<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TBufferInfo")
            .field("info", &self.info)
            .field("alloc_info", &"VmaAllocationCreateInfo { ... }") // VMA struct doesn't impl Debug
            .finish()
    }
}

/// Default implementation.
impl<'a> Default for TBufferInfo<'a> {
    fn default() -> Self {
        Self {
            info: vk::BufferCreateInfo {
                s_type: vk::StructureType::BUFFER_CREATE_INFO,
                ..Default::default()
            },
            alloc_info: vma::AllocationCreateInfo::default(),
            _marker: PhantomData,
        }
    }
}

/// Wrapper for vk::ImageCreateInfo that includes VMA info.
/// Generic over lifetime 'a due to ash::vk::ImageCreateInfo<'a>.
#[derive(Clone)]
pub struct TImageInfo<'a> {
    pub info: vk::ImageCreateInfo<'a>,
    pub alloc_info: vma::AllocationCreateInfo,
    _marker: PhantomData<&'a ()>,
}

/// Manual Debug impl as vma::AllocationCreateInfo doesn't impl Debug.
impl<'a> fmt::Debug for TImageInfo<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TImageInfo")
            .field("info", &self.info)
            .field("alloc_info", &"VmaAllocationCreateInfo { ... }") // VMA struct doesn't impl Debug
            .finish()
    }
}

/// Default implementation.
impl<'a> Default for TImageInfo<'a> {
    fn default() -> Self {
        Self {
            info: vk::ImageCreateInfo {
                s_type: vk::StructureType::IMAGE_CREATE_INFO,
                ..Default::default()
            },
            alloc_info: vma::AllocationCreateInfo::default(),
            _marker: PhantomData,
        }
    }
}

// --- Corresponds to helper functions in util.cpp ---
pub mod helper {
    use super::*;

    // We can return 'static lifetime here because the builders
    // don't borrow any data.
    pub fn storage_buffer_info(
        size: vk::DeviceSize,
        memory_type: vma::MemoryUsage,
    ) -> TBufferInfo<'static> {
        let usage = vk::BufferUsageFlags::STORAGE_BUFFER
            | vk::BufferUsageFlags::TRANSFER_DST
            | vk::BufferUsageFlags::TRANSFER_SRC;

        let info = vk::BufferCreateInfo {
            s_type: vk::StructureType::BUFFER_CREATE_INFO,
            size,
            usage,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            ..Default::default()
        };

        let alloc_info = vma::AllocationCreateInfo {
            usage: memory_type,
            ..Default::default()
        };

        TBufferInfo {
            info,
            alloc_info,
            _marker: PhantomData,
        }
    }

    pub fn sampled_image_info(
        width: u32,
        height: u32,
        format: vk::Format,
        usage: vk::ImageUsageFlags,
    ) -> TImageInfo<'static> {
        let info = vk::ImageCreateInfo {
            s_type: vk::StructureType::IMAGE_CREATE_INFO,
            image_type: vk::ImageType::TYPE_2D,
            extent: vk::Extent3D {
                width,
                height,
                depth: 1,
            },
            mip_levels: 1,
            array_layers: 1,
            format,
            tiling: vk::ImageTiling::OPTIMAL,
            initial_layout: vk::ImageLayout::UNDEFINED,
            usage: usage | vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
            samples: vk::SampleCountFlags::TYPE_1,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            ..Default::default()
        };

        let alloc_info = vma::AllocationCreateInfo {
            required_flags: vk::MemoryPropertyFlags::DEVICE_LOCAL,
            ..Default::default()
        };

        TImageInfo {
            info,
            alloc_info,
            _marker: PhantomData,
        }
    }

    pub fn storage_image_info(
        width: u32,
        height: u32,
        format: vk::Format,
        memory_type: vma::MemoryUsage,
        usage: vk::ImageUsageFlags,
    ) -> TImageInfo<'static> {
        let info = vk::ImageCreateInfo {
            s_type: vk::StructureType::IMAGE_CREATE_INFO,
            image_type: vk::ImageType::TYPE_2D,
            extent: vk::Extent3D {
                width,
                height,
                depth: 1,
            },
            mip_levels: 1,
            array_layers: 1,
            format,
            tiling: vk::ImageTiling::OPTIMAL,
            initial_layout: vk::ImageLayout::UNDEFINED,
            usage: usage | vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::TRANSFER_SRC,
            samples: vk::SampleCountFlags::TYPE_1,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            ..Default::default()
        };

        let alloc_info = vma::AllocationCreateInfo {
            required_flags: vk::MemoryPropertyFlags::DEVICE_LOCAL,
            usage: memory_type,
            ..Default::default()
        };

        TImageInfo {
            info,
            alloc_info,
            _marker: PhantomData,
        }
    }

    pub fn depth_image_info(
        width: u32,
        height: u32,
        format: vk::Format,
    ) -> vk::ImageCreateInfo<'static> {
        vk::ImageCreateInfo {
            s_type: vk::StructureType::IMAGE_CREATE_INFO,
            image_type: vk::ImageType::TYPE_2D,
            extent: vk::Extent3D {
                width,
                height,
                depth: 1,
            },
            mip_levels: 1,
            array_layers: 1,
            format,
            tiling: vk::ImageTiling::OPTIMAL,
            initial_layout: vk::ImageLayout::UNDEFINED,
            usage: vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            samples: vk::SampleCountFlags::TYPE_1,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            ..Default::default()
        }
    }
}

// --- Command Buffer Wrappers ---

/// Wrapper for a graphics command buffer that begins/ends rendering.
pub struct GraphicsCommandBuffer<'a> {
    cmd: &'a Device,
    buffer: vk::CommandBuffer,
}

impl<'a> GraphicsCommandBuffer<'a> {
    pub fn new(cmd: &'a Device, buffer: vk::CommandBuffer, info: &vk::RenderingInfo) -> Self {
        unsafe {
            cmd.cmd_begin_rendering(buffer, info);
        }
        Self { cmd, buffer }
    }

    pub fn bind_pipeline(&self, pipeline: vk::Pipeline) {
        unsafe {
            self.cmd
                .cmd_bind_pipeline(self.buffer, vk::PipelineBindPoint::GRAPHICS, pipeline);
        }
    }

    pub fn push_constants(
        &self,
        layout: vk::PipelineLayout,
        stage_flags: vk::ShaderStageFlags,
        offset: u32,
        data: &[u8],
    ) {
        unsafe {
            self.cmd
                .cmd_push_constants(self.buffer, layout, stage_flags, offset, data);
        }
    }

    pub fn set_viewport(&self, viewport: &vk::Viewport) {
        unsafe {
            self.cmd
                .cmd_set_viewport(self.buffer, 0, std::slice::from_ref(viewport));
        }
    }

    pub fn set_scissor(&self, scissor: &vk::Rect2D) {
        unsafe {
            self.cmd
                .cmd_set_scissor(self.buffer, 0, std::slice::from_ref(scissor));
        }
    }

    pub fn draw(
        &self,
        vertex_count: u32,
        instance_count: u32,
        first_vertex: u32,
        first_instance: u32,
    ) {
        unsafe {
            self.cmd.cmd_draw(
                self.buffer,
                vertex_count,
                instance_count,
                first_vertex,
                first_instance,
            );
        }
    }

    pub fn draw_indirect(
        &self,
        buffer: vk::Buffer,
        offset: vk::DeviceSize,
        draw_count: u32,
        stride: u32,
    ) {
        unsafe {
            self.cmd
                .cmd_draw_indirect(self.buffer, buffer, offset, draw_count, stride);
        }
    }
}

impl<'a> Drop for GraphicsCommandBuffer<'a> {
    fn drop(&mut self) {
        unsafe {
            self.cmd.cmd_end_rendering(self.buffer);
        }
    }
}

/// Wrapper for a compute command buffer.
pub struct ComputeCommandBuffer<'a> {
    cmd: &'a Device,
    buffer: vk::CommandBuffer,
}

impl<'a> ComputeCommandBuffer<'a> {
    pub fn new(cmd: &'a Device, buffer: vk::CommandBuffer) -> Self {
        Self { cmd, buffer }
    }

    pub fn bind_pipeline(&self, pipeline: vk::Pipeline) {
        unsafe {
            self.cmd
                .cmd_bind_pipeline(self.buffer, vk::PipelineBindPoint::COMPUTE, pipeline);
        }
    }

    pub fn push_constants(
        &self,
        layout: vk::PipelineLayout,
        stage_flags: vk::ShaderStageFlags,
        offset: u32,
        data: &[u8],
    ) {
        unsafe {
            self.cmd
                .cmd_push_constants(self.buffer, layout, stage_flags, offset, data);
        }
    }

    pub fn dispatch(&self, group_count_x: u32, group_count_y: u32, group_count_z: u32) {
        unsafe {
            self.cmd
                .cmd_dispatch(self.buffer, group_count_x, group_count_y, group_count_z);
        }
    }
}

/// Wrapper for a transfer command buffer.
pub struct TransferCommandBuffer<'a> {
    cmd: &'a Device,
    buffer: vk::CommandBuffer,
}

impl<'a> TransferCommandBuffer<'a> {
    pub fn new(cmd: &'a Device, buffer: vk::CommandBuffer) -> Self {
        Self { cmd, buffer }
    }

    pub fn copy_buffer(
        &self,
        src: vk::Buffer,
        dst: vk::Buffer,
        size: vk::DeviceSize,
        src_offset: vk::DeviceSize,
        dst_offset: vk::DeviceSize,
    ) {
        let region = vk::BufferCopy {
            src_offset,
            dst_offset,
            size,
        };
        unsafe {
            self.cmd
                .cmd_copy_buffer(self.buffer, src, dst, std::slice::from_ref(&region));
        }
    }

    pub fn copy_buffer_to_image(
        &self,
        src: vk::Buffer,
        dst: vk::Image,
        dst_layout: vk::ImageLayout,
        region: &vk::BufferImageCopy,
    ) {
        unsafe {
            self.cmd.cmd_copy_buffer_to_image(
                self.buffer,
                src,
                dst,
                dst_layout,
                std::slice::from_ref(region),
            );
        }
    }

    pub fn copy_image_to_buffer(
        &self,
        src: vk::Image,
        src_layout: vk::ImageLayout,
        dst: vk::Buffer,
        region: &vk::BufferImageCopy,
    ) {
        unsafe {
            self.cmd.cmd_copy_image_to_buffer(
                self.buffer,
                src,
                src_layout,
                dst,
                std::slice::from_ref(region),
            );
        }
    }

    pub fn copy_image(
        &self,
        src: vk::Image,
        src_layout: vk::ImageLayout,
        dst: vk::Image,
        dst_layout: vk::ImageLayout,
        region: &vk::ImageCopy,
    ) {
        unsafe {
            self.cmd.cmd_copy_image(
                self.buffer,
                src,
                src_layout,
                dst,
                dst_layout,
                std::slice::from_ref(region),
            );
        }
    }

    pub fn blit_image(
        &self,
        src: vk::Image,
        src_layout: vk::ImageLayout,
        dst: vk::Image,
        dst_layout: vk::ImageLayout,
        region: &vk::ImageBlit,
        filter: vk::Filter,
    ) {
        unsafe {
            self.cmd.cmd_blit_image(
                self.buffer,
                src,
                src_layout,
                dst,
                dst_layout,
                std::slice::from_ref(region),
                filter,
            );
        }
    }
}
