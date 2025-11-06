//! Translation of queue.hpp/cpp
//! Uses 'ash' types.

use super::util::vk_check;
use ash::{vk, Device};
use bitflags::bitflags;
use bitvec::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    #[repr(transparent)]
    pub struct QueueCapability: u32 {
        const Graphics = 1 << 0;
        const Compute  = 1 << 1;
        const Transfer = 1 << 2;
    }
}

pub type QueueHandle = u32;

#[derive(Debug, Clone, Copy)]
pub struct QueueInfo {
    pub queue: vk::Queue,
    pub family_index: u32,
    pub capabilities: QueueCapability,
}

/// A stateful, thread-safe manager for Vulkan queues.
#[derive(Debug, Default)]
pub struct QueueManager {
    queues: RwLock<Vec<QueueInfo>>,
}

impl QueueManager {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn register_queue(
        &self,
        queue: vk::Queue,
        family_index: u32,
        capabilities: QueueCapability,
    ) {
        let mut queues = self.queues.write().unwrap();
        queues.push(QueueInfo {
            queue,
            family_index,
            capabilities,
        });
    }

    pub fn get_queue(&self, handle: QueueHandle) -> Option<QueueInfo> {
        let queues = self.queues.read().unwrap();
        queues.get(handle as usize).copied()
    }
}

/// Corresponds to CommandBufferManager
pub struct CommandBufferManager {
    device: Arc<Device>,
    queues: Mutex<HashMap<QueueHandle, QueuePool>>,
}

struct CommandBufferEntry {
    pool: vk::CommandPool,
    buffer: vk::CommandBuffer,
}

struct QueuePool {
    buffers: Vec<CommandBufferEntry>,
    in_use_mask: BitVec<u64>,
    queue_handle: QueueHandle,
}

impl CommandBufferManager {
    pub fn new(device: Arc<Device>) -> Self {
        Self {
            device,
            queues: Mutex::new(HashMap::new()),
        }
    }

    pub fn request(
        &self,
        queue_manager: &QueueManager,
        queue_handle: QueueHandle,
    ) -> vk::CommandBuffer {
        let mut pools = self.queues.lock().unwrap();
        
        // --- FIX: Renamed variable to `queue_pool` ---
        let queue_pool = pools.entry(queue_handle).or_insert_with(|| QueuePool {
            buffers: Vec::new(),
            in_use_mask: BitVec::new(),
            queue_handle,
        });

        // Use `queue_pool` to refer to the struct
        if let Some(index) = queue_pool.in_use_mask.first_zero() {
            queue_pool.in_use_mask.set(index, true);
            queue_pool.buffers[index].buffer
        } else {
            let queue_info = queue_manager.get_queue(queue_handle).expect("Invalid queue handle");

            let pool_info = vk::CommandPoolCreateInfo::default()
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                .queue_family_index(queue_info.family_index);
            
            // This `pool` (vk::CommandPool) no longer conflicts
            let pool = vk_check!(unsafe { self.device.create_command_pool(&pool_info, None) });

            let alloc_info = vk::CommandBufferAllocateInfo::default()
                .command_pool(pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(1);
            let buffer = vk_check!(unsafe { self.device.allocate_command_buffers(&alloc_info) })[0];

            // Use `queue_pool` to refer to the struct
            let index = queue_pool.buffers.len();
            queue_pool.buffers.push(CommandBufferEntry { pool, buffer });
            queue_pool.in_use_mask.resize(index + 1, false);
            queue_pool.in_use_mask.set(index, true);
            buffer
        }
    }

    pub fn reset_all(&self) {
        let pools = self.queues.lock().unwrap();
        for (_, pool) in pools.iter() {
            for entry in &pool.buffers {
                vk_check!(unsafe { self.device.reset_command_pool(entry.pool, vk::CommandPoolResetFlags::empty()) });
            }
        }
    }
}

impl Drop for CommandBufferManager {
    fn drop(&mut self) {
        let pools = self.queues.lock().unwrap();
        for (_, pool) in pools.iter() {
            for entry in &pool.buffers {
                unsafe {
                    self.device.destroy_command_pool(entry.pool, None);
                }
            }
        }
    }
}

