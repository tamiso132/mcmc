//! Translation of bindless.hpp
//! Uses 'ash' types.
//!
//! CORRECTED: Replaced the `.builder()` pattern with the
//! `::default()` struct initialization pattern for compatibility.
//!
//! CORRECTED: Fixed compile-time error in const METAS by combining
//! the underlying bitflag values using the `::from_raw` constructor,
//! which is allowed in const contexts.

use super::util::vk_check;
use ash::{Device, vk};
use std::sync::Arc;

#[derive(Debug, Clone, Copy)]
pub enum BindlessType {
    CombinedImageSampler,
    StorageImage,
    StorageBuffer,
    UniformBuffer,
}

impl From<BindlessType> for usize {
    fn from(val: BindlessType) -> Self {
        val as usize
    }
}

struct DescriptorMeta {
    descriptor_type: vk::DescriptorType,
    stage_flags: vk::ShaderStageFlags,
}

// FIX: We must combine flags using their raw value (via ::from_raw)
// because the `|` operator is not available in const context.
const SHADER_STAGES: vk::ShaderStageFlags = vk::ShaderStageFlags::from_raw(
    vk::ShaderStageFlags::COMPUTE.as_raw()
        | vk::ShaderStageFlags::FRAGMENT.as_raw()
        | vk::ShaderStageFlags::VERTEX.as_raw(),
);

const METAS: [DescriptorMeta; 4] = [
    DescriptorMeta {
        descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
        stage_flags: SHADER_STAGES,
    },
    DescriptorMeta {
        descriptor_type: vk::DescriptorType::STORAGE_IMAGE,
        stage_flags: SHADER_STAGES,
    },
    DescriptorMeta {
        descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
        stage_flags: SHADER_STAGES,
    },
    DescriptorMeta {
        descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
        stage_flags: SHADER_STAGES,
    },
];

pub struct BindlessDescriptors {
    device: Arc<Device>,
    pub pool: vk::DescriptorPool,
    pub layout: vk::DescriptorSetLayout,
    pub set: vk::DescriptorSet,
    pub pipeline_layout: vk::PipelineLayout,
}

impl BindlessDescriptors {
    pub fn new(device: Arc<Device>) -> Self {
        // --- Use ::default() pattern ---
        let bindings: Vec<vk::DescriptorSetLayoutBinding> = METAS
            .iter()
            .enumerate()
            .map(|(i, meta)| vk::DescriptorSetLayoutBinding {
                stage_flags: meta.stage_flags,
                binding: i as u32,
                descriptor_count: 65535, // Max descriptors
                descriptor_type: meta.descriptor_type,
                ..Default::default()
            })
            .collect();

        let layout_info = vk::DescriptorSetLayoutCreateInfo {
            s_type: vk::StructureType::DESCRIPTOR_SET_LAYOUT_CREATE_INFO,
            p_bindings: bindings.as_ptr(),
            binding_count: bindings.len() as u32,
            ..Default::default()
        };
        let layout = vk_check!(unsafe { device.create_descriptor_set_layout(&layout_info, None) });

        let pool_sizes: Vec<vk::DescriptorPoolSize> = bindings
            .iter()
            .map(|b| vk::DescriptorPoolSize {
                ty: b.descriptor_type,
                descriptor_count: b.descriptor_count,
            })
            .collect();

        let pool_info = vk::DescriptorPoolCreateInfo {
            s_type: vk::StructureType::DESCRIPTOR_POOL_CREATE_INFO,
            p_pool_sizes: pool_sizes.as_ptr(),
            pool_size_count: pool_sizes.len() as u32,
            max_sets: 1,
            ..Default::default()
        };
        let pool = vk_check!(unsafe { device.create_descriptor_pool(&pool_info, None) });

        let set_layouts = [layout];
        let set_alloc_info = vk::DescriptorSetAllocateInfo {
            s_type: vk::StructureType::DESCRIPTOR_SET_ALLOCATE_INFO,
            descriptor_pool: pool,
            p_set_layouts: set_layouts.as_ptr(),
            descriptor_set_count: 1,
            ..Default::default()
        };
        let set = vk_check!(unsafe { device.allocate_descriptor_sets(&set_alloc_info) })[0];

        let range = vk::PushConstantRange {
            size: 128,
            stage_flags: SHADER_STAGES,
            offset: 0,
        };

        let pipeline_layout_info = vk::PipelineLayoutCreateInfo {
            s_type: vk::StructureType::PIPELINE_LAYOUT_CREATE_INFO,
            p_set_layouts: set_layouts.as_ptr(),
            set_layout_count: 1,
            p_push_constant_ranges: &range,
            push_constant_range_count: 1,
            ..Default::default()
        };

        let pipeline_layout =
            vk_check!(unsafe { device.create_pipeline_layout(&pipeline_layout_info, None) });

        Self {
            device,
            pool,
            layout,
            set,
            pipeline_layout,
        }
    }

    pub fn update(
        &self,
        ty: BindlessType,
        index: u32,
        buf_info: Option<&[vk::DescriptorBufferInfo]>,
        img_info: Option<&[vk::DescriptorImageInfo]>,
    ) {
        let meta = &METAS[ty as usize];

        // --- Use ::default() pattern ---
        let mut write = vk::WriteDescriptorSet {
            s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
            dst_set: self.set,
            dst_binding: ty as u32,
            dst_array_element: index,
            descriptor_type: meta.descriptor_type,
            descriptor_count: 1, // Must be 1
            ..Default::default()
        };

        if let Some(buf) = buf_info {
            write.p_buffer_info = buf.as_ptr();
            write.descriptor_count = buf.len() as u32;
        }
        if let Some(img) = img_info {
            write.p_image_info = img.as_ptr();
            write.descriptor_count = img.len() as u32;
        }

        unsafe {
            self.device
                .update_descriptor_sets(std::slice::from_ref(&write), &[]);
        }
    }
}

impl Drop for BindlessDescriptors {
    fn drop(&mut self) {
        unsafe {
            self.device
                .destroy_pipeline_layout(self.pipeline_layout, None);
            self.device.destroy_descriptor_pool(self.pool, None);
            self.device.destroy_descriptor_set_layout(self.layout, None);
        }
    }
}
