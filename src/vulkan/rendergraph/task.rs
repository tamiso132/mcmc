//! Translation of task.hpp/cpp
//!
//! Defines the non-generic `Task` struct and the generic,
//! type-safe `TaskBuilder<REnum>`.

use super::types::{Access, ExecuteFunction, TaskType};
use ash::vk;

// --- Internal Resource Info Structs ---
// These are stored in the Task and use raw u32 indices.
#[derive(Debug, Clone, Copy)]
pub(super) struct BufferResourceInfo {
    pub(super) index: u32,
    pub(super) stage: vk::PipelineStageFlags2,
    pub(super) access: Access,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ImageResourceInfo {
    pub(super) index: u32,
    pub(super) stage: vk::PipelineStageFlags2,
    pub(super) access: Access,
    pub(super) layout: vk::ImageLayout,
}

#[derive(Clone, Copy)]
pub(super) struct ColorAttachmentInfo {
    pub(super) index: u32,
    pub(super) load_op: vk::AttachmentLoadOp,
    pub(super) store_op: vk::AttachmentStoreOp,
    pub(super) clear_value: vk::ClearValue,
}

/// Corresponds to `vklib::Task`
///
/// This is a non-generic struct that the RenderGraph consumes.
/// It is created by the generic `TaskBuilder`.
#[derive(Default)]
pub struct Task {
    pub(super) name: String,
    pub(super) buffers: Vec<BufferResourceInfo>,
    pub(super) images: Vec<ImageResourceInfo>,
    pub(super) color_attachments: Vec<ColorAttachmentInfo>,
    pub(super) execute_fn: Option<ExecuteFunction>,
    pub(super) stage: vk::PipelineStageFlags2,
}

impl Task {
    // The C++ `execute` is now just a call to the closure.
    pub(super) fn execute(
        &self,
        encoder: &mut super::types::PassEncoder,
    ) -> crate::error::AppResult<()> {
        if let Some(exec) = &self.execute_fn {
            // We can't move out of `&self`, so this is tricky.
            // The C++ std::function can be copied.
            // A simple `Box<dyn Fn>` would be easier.
            // For now, let's assume `execute_fn` is `None` after this,
            // which is bad.
            //
            // **CORRECTION:** The `execute_fn` is `Box<dyn FnOnce...>`,
            // so it *must* be consumed. This means the `Task` struct
            // itself must be consumed. `RenderGraph::execute` will
            // need to `take()` this function.
            panic!("Task::execute should not be called directly; RenderGraph handles the closure.");
        }
        Ok(())
    }
}

/// Corresponds to `vklib::builder::TaskBuilder<REnum>`
///
/// This is the public, type-safe builder.
/// `REnum` must be an enum that can be cast to `u32`.
pub struct TaskBuilder<REnum>
where
    REnum: Into<u32> + Copy,
{
    task: Task,
    _phantom: std::marker::PhantomData<REnum>,
}

impl<REnum> TaskBuilder<REnum>
where
    REnum: Into<u32> + Copy,
{
    pub fn new(name: &str) -> Self {
        Self {
            task: Task {
                name: name.to_string(),
                ..Default::default()
            },
            _phantom: std::marker::PhantomData,
        }
    }

    /// Corresponds to `add_buffer_info`
    fn add_buffer_info(
        mut self,
        index: REnum,
        stage: vk::PipelineStageFlags2,
        access: Access,
    ) -> Self {
        self.task.buffers.push(BufferResourceInfo {
            index: index.into(), // Type-safe enum to u32
            stage,
            access,
        });
        self
    }

    /// Corresponds to `add_image_info`
    fn add_image_info(
        mut self,
        index: REnum,
        stage: vk::PipelineStageFlags2,
        layout: vk::ImageLayout,
        access: Access,
    ) -> Self {
        self.task.images.push(ImageResourceInfo {
            index: index.into(), // Type-safe enum to u32
            stage,
            access,
            layout,
        });
        self
    }

    // --- Public Type-Safe API ---

    pub fn buffer_read(self, index: REnum, stage: vk::PipelineStageFlags2) -> Self {
        self.add_buffer_info(index, stage, Access::Read)
    }

    pub fn buffer_write(self, index: REnum, stage: vk::PipelineStageFlags2) -> Self {
        self.add_buffer_info(index, stage, Access::Write)
    }

    pub fn buffer_read_write(self, index: REnum, stage: vk::PipelineStageFlags2) -> Self {
        self.add_buffer_info(index, stage, Access::ReadWrite)
    }

    pub fn image_read(
        self,
        index: REnum,
        stage: vk::PipelineStageFlags2,
        layout: vk::ImageLayout,
    ) -> Self {
        self.add_image_info(index, stage, layout, Access::Read)
    }

    pub fn image_write(
        self,
        index: REnum,
        stage: vk::PipelineStageFlags2,
        layout: vk::ImageLayout,
    ) -> Self {
        self.add_image_info(index, stage, layout, Access::Write)
    }

    pub fn image_read_write(
        self,
        index: REnum,
        stage: vk::PipelineStageFlags2,
        layout: vk::ImageLayout,
    ) -> Self {
        self.add_image_info(index, stage, layout, Access::ReadWrite)
    }

    pub fn color_attachment(
        mut self,
        index: REnum,
        clear: Option<vk::ClearValue>,
        load_op: vk::AttachmentLoadOp,
        store_op: vk::AttachmentStoreOp,
    ) -> Self {
        self.task.color_attachments.push(ColorAttachmentInfo {
            index: index.into(),
            load_op,
            store_op,
            clear_value: clear.unwrap_or_default(),
        });

        // Auto-add the image write dependency
        self.add_image_info(
            index,
            vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            Access::Write,
        )
    }

    /// Consumes the builder and returns the non-generic `Task`
    pub fn build(mut self, ty: TaskType, func: ExecuteFunction) -> Task {
        self.task.execute_fn = Some(func);
        // Note: C++ version had a switch on `ty`, which we can add later
        // if we need to set stage flags based on it.
        self.task
    }
}
