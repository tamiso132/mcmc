//! Translation of rendergraph.hpp/cpp
//!
//! Contains the core `RenderGraph` struct and its dependency
//! resolution (`compile`) and execution (`execute`) logic.
//!
//! FIX: Removed lifetimes 'a from RenderGraph, Subgraph, and TaskInfo
//! to resolve lifetime variance issues in RenderGraphConfigurator.
//! Barriers are now explicitly 'static.

use super::configurator::RenderGraphConfigurator;
use super::task::{BufferResourceInfo, ImageResourceInfo, Task};
use super::types::{Access, PassEncoder, TaskType, get_access_flags};
use crate::error::{AppError, AppResult};
use crate::vulkan;
use crate::vulkan::queue::QueueManager;
use ash::{Device, vk};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLockReadGuard};
use vulkan::queue::CommandBufferManager;
use vulkan::resource::{BufferData, ImageData, ResourceHandle, ResourceManager};
use vulkan::util::vk_check;

// --- Internal Structs ---

/// Corresponds to `TaskNode` in rendergraph.cpp
struct TaskNode {
    task_index: u32,
    dependents: Vec<u32>,
    counter: i32,
}

/// Corresponds to `Subgraph` in rendergraph.hpp
struct Subgraph {
    tasks: Vec<TaskInfo>, // <-- Removed 'a
    dependents: Vec<u32>,
    // queue: InternalQueueHandle, // TODO
    submit_order: u32,
}

/// Corresponds to `Subgraph::TaskInfo`
struct TaskInfo {
    id: u32,                                                 // Index into the graph's m_tasks vec
    image_barriers: Vec<vk::ImageMemoryBarrier2<'static>>,   // <-- Changed to 'static
    buffer_barriers: Vec<vk::BufferMemoryBarrier2<'static>>, // <-- Changed to 'static
}

/// Corresponds to `ResourceSyncInfo`
#[derive(Debug, Clone)]
struct ResourceSyncInfo {
    last_layout_write: vk::ImageLayout,
    last_stage_write: vk::PipelineStageFlags2,
    last_access_write: vk::AccessFlags2,
    last_layout: vk::ImageLayout,
    last_stage: vk::PipelineStageFlags2,
    last_access: vk::AccessFlags2,
    read_stages: Vec<(vk::PipelineStageFlags2, vk::ImageLayout)>,
    is_first: bool,
}

impl ResourceSyncInfo {
    fn new(
        stage: vk::PipelineStageFlags2,
        access: vk::AccessFlags2,
        layout: vk::ImageLayout,
    ) -> Self {
        Self {
            last_layout_write: layout,
            last_stage_write: stage,
            last_access_write: access,
            last_layout: layout, // C++ version didn't init this, but we should
            last_stage: stage,   // C++ version didn't init this
            last_access: access, // C++ version didn't init this
            read_stages: Vec::new(),
            is_first: true,
        }
    }

    fn update_latest_write(
        &mut self,
        stage: vk::PipelineStageFlags2,
        access: vk::AccessFlags2,
        layout: vk::ImageLayout,
    ) {
        self.last_stage_write = stage;
        self.last_access_write = access;
        self.last_layout_write = layout;
        self.read_stages.clear();
        self.update_latest(stage, access, layout);
    }

    fn update_latest(
        &mut self,
        stage: vk::PipelineStageFlags2,
        access: vk::AccessFlags2,
        layout: vk::ImageLayout,
    ) {
        self.last_access = access;
        self.last_stage = stage;
        self.last_layout = layout;
        self.is_first = false;
    }

    fn has_read_already(&mut self, stage: vk::PipelineStageFlags2) -> bool {
        if !self.read_stages.iter().any(|(s, _)| *s == stage) {
            self.read_stages.push((stage, vk::ImageLayout::UNDEFINED));
            return false;
        }
        true
    }
}

/// Corresponds to `ResHandles`
struct ResHandles {
    res_manager: Arc<ResourceManager>,
    /// Map of internal graph index (u32) -> actual ResourceHandle
    res_handles: HashMap<u32, ResourceHandle>,
    /// Map of internal graph index (u32) -> sync state
    res_to_usage: HashMap<u32, ResourceSyncInfo>,
}

impl ResHandles {
    fn new(res_manager: Arc<ResourceManager>) -> Self {
        Self {
            res_manager,
            res_handles: HashMap::new(),
            res_to_usage: HashMap::new(),
        }
    }

    fn import_handle(&mut self, index: u32, handle: ResourceHandle, sync_info: ResourceSyncInfo) {
        self.res_handles.insert(index, handle);
        self.res_to_usage.insert(index, sync_info);
    }

    fn replace_handle(&mut self, index: u32, new_handle: ResourceHandle) {
        self.res_handles.insert(index, new_handle);
    }

    /// Gets the sync info for a resource by its *internal graph index*
    fn get_sync_info_mut(&mut self, index: u32) -> &mut ResourceSyncInfo {
        self.res_to_usage
            .get_mut(&index)
            .expect("Resource not imported")
    }

    /// Gets the *actual* ResourceHandle by its *internal graph index*
    fn get_handle(&self, index: u32) -> ResourceHandle {
        self.res_handles
            .get(&index)
            .copied()
            .expect("Resource not imported")
    }
}

/// Corresponds to `vklib::RenderGraph`
pub struct RenderGraph {
    // <-- Removed 'a
    device: Arc<Device>,
    handles: ResHandles,
    tasks: Vec<Task>,
    subgraphs: Vec<Subgraph>, // <-- Removed 'a
    queue_mng: QueueManager,
    cmd_manager: Arc<CommandBufferManager>,
    // TODO: Add queue info
}

impl RenderGraph {
    // <-- Removed 'a
    pub fn new(
        device: Arc<Device>,
        res_manager: Arc<ResourceManager>,
        cmd_manager: Arc<CommandBufferManager>,
    ) -> Self {
        Self {
            device,
            handles: ResHandles::new(res_manager),
            tasks: Vec::new(),
            subgraphs: Vec::new(),
            cmd_manager,
            queue_mng: QueueManager::new(),
        }
    }

    /// Corresponds to `RenderGraph::configure<REnum>()`
    pub fn configure<REnum>(&mut self) -> RenderGraphConfigurator<REnum>
    where
        REnum: Into<u32> + Copy,
    {
        RenderGraphConfigurator::new(self) // <-- Removed 'a
    }

    /// Corresponds to `add_task`
    pub fn add_task(&mut self, task: Task) -> &mut Self {
        self.tasks.push(task);
        self
    }

    // --- Private Internal API (called by Configurator) ---

    pub(super) fn import_buffer_internal(&mut self, index: u32, handle: ResourceHandle) {
        let sync_info = ResourceSyncInfo::new(
            vk::PipelineStageFlags2::TOP_OF_PIPE,
            vk::AccessFlags2::NONE,
            vk::ImageLayout::UNDEFINED, // Buffers don't use layouts
        );
        self.handles.import_handle(index, handle, sync_info);
    }

    pub(super) fn import_image_internal(
        &mut self,
        index: u32,
        handle: ResourceHandle,
        init_layout: vk::ImageLayout,
    ) {
        let sync_info = ResourceSyncInfo::new(
            vk::PipelineStageFlags2::TOP_OF_PIPE,
            vk::AccessFlags2::NONE,
            init_layout,
        );
        self.handles.import_handle(index, handle, sync_info);
    }

    pub(super) fn replace_resource_internal(&mut self, index: u32, new_res: ResourceHandle) {
        self.handles.replace_handle(index, new_res);
    }

    /// Corresponds to `compile()`
    pub fn compile(&mut self) -> AppResult<()> {
        let task_count = self.tasks.len();
        if task_count == 0 {
            return Ok(());
        }

        let mut nodes: Vec<TaskNode> = (0..task_count)
            .map(|i| TaskNode {
                task_index: i as u32,
                dependents: Vec::new(),
                counter: 0,
            })
            .collect();

        let mut next_reader: HashMap<u32, Vec<u32>> = HashMap::new();
        let mut next_writer: HashMap<u32, Vec<u32>> = HashMap::new();

        // 1. Build dependency graph (reverse iteration)
        for i in (0..task_count).rev() {
            // let node = &mut nodes[i];
            let task = &self.tasks[nodes[i].task_index as usize];

            let mut handle_resource = |index: u32, access: Access| {
                if access.contains(Access::Write) {
                    // Write-After-Read
                    if let Some(future_reads) = next_reader.get(&index) {
                        for &fut_read in future_reads {
                            nodes[i].dependents.push(fut_read);
                            nodes[fut_read as usize].counter += 1;
                        }
                    }
                    // Write-After-Write
                    if let Some(future_writes) = next_writer.get(&index) {
                        for &fut_write in future_writes {
                            nodes[i].dependents.push(fut_write);
                            nodes[fut_write as usize].counter += 1;
                        }
                    }
                    next_writer.entry(index).or_default().push(i as u32);
                } else if access.contains(Access::Read) {
                    // Read-After-Write
                    if let Some(future_writes) = next_writer.get(&index) {
                        for &fut_write in future_writes {
                            nodes[i].dependents.push(fut_write);
                            nodes[fut_write as usize].counter += 1;
                        }
                    }
                    next_reader.entry(index).or_default().push(i as u32);
                }
            };

            for res in &task.buffers {
                handle_resource(res.index, res.access);
            }
            for res in &task.images {
                handle_resource(res.index, res.access);
            }
        }

        // 2. Build Subgraphs and Barriers
        let mut subgraphs = Vec::new();
        let mut task_to_subgraph: HashMap<u32, u32> = HashMap::new();

        // Get resource data guards ONCE. This is safe because we only
        // read from them during this phase.
        let temp_res = self.handles.res_manager.clone();
        let buffers_guard = temp_res.get_buffers();
        let images_guard = temp_res.get_images();

        for i in 0..nodes.len() {
            if nodes[i].counter == 0 {
                // This node is a root of a subgraph
                let mut subgraph = Subgraph {
                    tasks: Vec::new(),
                    dependents: Vec::new(),
                    submit_order: 0,
                };
                let mut current_task_index = i;

                loop {
                    let task_id = nodes[current_task_index].task_index;
                    let task = &self.tasks[task_id as usize];
                    let mut task_info = TaskInfo {
                        id: task_id,
                        image_barriers: Vec::new(),
                        buffer_barriers: Vec::new(),
                    };

                    // --- Handle Buffer Barriers ---
                    for res in &task.buffers {
                        let handle = self.handles.get_handle(res.index);
                        let meta = &buffers_guard[handle.id as usize];
                        let current_ver = self.handles.get_sync_info_mut(res.index);

                        let dst_access = get_access_flags(res.stage, res.access);

                        if res.access.contains(Access::Write) {
                            if !current_ver.is_first {
                                // WAW or RAW
                                task_info.buffer_barriers.push(
                                    vk::BufferMemoryBarrier2::default() // <-- This is 'static
                                        .src_stage_mask(current_ver.last_stage)
                                        .src_access_mask(current_ver.last_access)
                                        .dst_stage_mask(res.stage)
                                        .dst_access_mask(dst_access)
                                        .buffer(meta.handle)
                                        .offset(meta.offset)
                                        .size(meta.size),
                                );
                            }
                            current_ver.update_latest_write(
                                res.stage,
                                dst_access,
                                vk::ImageLayout::UNDEFINED, // Layout n/a for buffers
                            );
                        } else if res.access.contains(Access::Read) {
                            if !current_ver.is_first && !current_ver.has_read_already(res.stage) {
                                // WAR
                                task_info.buffer_barriers.push(
                                    vk::BufferMemoryBarrier2::default() // <-- This is 'static
                                        .src_stage_mask(
                                            current_ver.last_stage_write,
                                        )
                                        .src_access_mask(
                                            current_ver.last_access_write,
                                        )
                                        .dst_stage_mask(res.stage)
                                        .dst_access_mask(dst_access)
                                        .buffer(meta.handle)
                                        .offset(meta.offset)
                                        .size(meta.size),
                                );
                            }
                            current_ver.update_latest(
                                res.stage,
                                dst_access,
                                vk::ImageLayout::UNDEFINED,
                            );
                        }
                    }

                    // --- Handle Image Barriers ---
                    for res in &task.images {
                        let handle = self.handles.get_handle(res.index);
                        let meta = &images_guard[handle.id as usize];
                        let current_ver = self.handles.get_sync_info_mut(res.index);

                        let dst_access = get_access_flags(res.stage, res.access);

                        if res.access.contains(Access::Write) {
                            if !current_ver.is_first {
                                task_info.image_barriers.push(
                                    vk::ImageMemoryBarrier2::default() // <-- This is 'static
                                        .src_stage_mask(current_ver.last_stage)
                                        .src_access_mask(current_ver.last_access)
                                        .old_layout(current_ver.last_layout)
                                        .dst_stage_mask(res.stage)
                                        .dst_access_mask(dst_access)
                                        .new_layout(res.layout)
                                        .image(meta.handle)
                                        .subresource_range(vk::ImageSubresourceRange {
                                            aspect_mask: meta.aspect,
                                            level_count: meta.mip_levels,
                                            layer_count: meta.array_layers,
                                            ..Default::default()
                                        }),
                                );
                            }
                            current_ver.update_latest_write(res.stage, dst_access, res.layout);
                        } else if res.access.contains(Access::Read) {
                            if !current_ver.is_first && !current_ver.has_read_already(res.stage) {
                                task_info.image_barriers.push(
                                    vk::ImageMemoryBarrier2::default() // <-- This is 'static
                                        .src_stage_mask(current_ver.last_stage_write)
                                        .src_access_mask(current_ver.last_access_write)
                                        .old_layout(current_ver.last_layout_write)
                                        .dst_stage_mask(res.stage)
                                        .dst_access_mask(dst_access)
                                        .new_layout(res.layout)
                                        .image(meta.handle)
                                        .subresource_range(vk::ImageSubresourceRange {
                                            aspect_mask: meta.aspect,
                                            level_count: meta.mip_levels,
                                            layer_count: meta.array_layers,
                                            ..Default::default()
                                        }),
                                );
                            }
                            current_ver.update_latest(res.stage, dst_access, res.layout);
                        }
                    }

                    nodes[current_task_index].counter = -1; // Mark as visited
                    subgraph.tasks.push(task_info);
                    task_to_subgraph.insert(task_id, subgraphs.len() as u32);

                    // Check dependents to continue or break chain
                    if nodes[current_task_index].dependents.len() > 1 {
                        // This task is a fan-out, end subgraph here
                        for &depend in &nodes[current_task_index].dependents.clone() {
                            nodes[depend as usize].counter -= 1;
                            subgraph.dependents.push(depend);
                        }
                        break;
                    }

                    if nodes[current_task_index].dependents.is_empty() {
                        // End of a chain
                        break;
                    }

                    let next_node_index = nodes[current_task_index].dependents[0] as usize;

                    if nodes[next_node_index].counter != 1 {
                        // Next node has multiple dependencies, so it's the
                        // start of a *different* subgraph. End this one.
                        nodes[next_node_index].counter -= 1;
                        subgraph.dependents.push(next_node_index as u32);
                        break;
                    }

                    // This is a simple 1:1 chain, continue subgraph
                    current_task_index = next_node_index;
                }
                subgraphs.push(subgraph);
            }
        }

        // 3. Remap task-based dependencies to subgraph-based dependencies
        for subgraph in &mut subgraphs {
            let mut subgraph_deps = HashSet::new();
            for task_dep_idx in &subgraph.dependents {
                let subgraph_idx = task_to_subgraph
                    .get(task_dep_idx)
                    .expect("Task has no subgraph");
                subgraph_deps.insert(*subgraph_idx);
            }
            subgraph.dependents = subgraph_deps.into_iter().collect();
        }

        self.subgraphs = subgraphs;
        Ok(())
    }

    /// Corresponds to `execute()`
    pub fn execute(&mut self) -> AppResult<()> {
        // TODO: This is a simplified execution that just runs all
        // subgraphs in order. A real implementation would use the
        // subgraph dependencies to build a submission order
        // and handle semaphores between queues.

        // For now, we assume a single (graphics) queue.
        // We'll use the CommandBufferManager from `queue.rs`.
        // We assume queue_handle 0 is our main graphics queue.
        const MAIN_QUEUE: u32 = 0;

        // Get resource data guards for the *entire* execution frame
        let buffers_guard = self.handles.res_manager.get_buffers();
        let images_guard = self.handles.res_manager.get_images();

        for subgraph in &self.subgraphs {
            let cmd = self.cmd_manager.request(&self.queue_mng, MAIN_QUEUE);
            let begin_info = vk::CommandBufferBeginInfo::default()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
            vk_check!(unsafe { self.device.begin_command_buffer(cmd, &begin_info) });

            for task_info in &subgraph.tasks {
                // 1. Apply all barriers for this task
                if !task_info.buffer_barriers.is_empty() || !task_info.image_barriers.is_empty() {
                    let dep_info = vk::DependencyInfo::default()
                        .buffer_memory_barriers(&task_info.buffer_barriers)
                        .image_memory_barriers(&task_info.image_barriers);
                    unsafe { self.device.cmd_pipeline_barrier2(cmd, &dep_info) };
                }

                // 2. Execute the task
                let task = self
                    .tasks
                    .get_mut(task_info.id as usize)
                    .expect("Task ID out of bounds");

                if let Some(exec_fn) = task.execute_fn.take() {
                    let mut encoder = PassEncoder {
                        cmd,
                        device: &self.device,
                        res_manager: &self.handles.res_manager,
                        buffers_guard: &buffers_guard,
                        images_guard: &images_guard,
                    };
                    // Run the user's closure
                    exec_fn(&mut encoder)?;
                }
            }

            vk_check!(unsafe { self.device.end_command_buffer(cmd) });

            // TODO: Submit to queue
            // let submit_info = vk::SubmitInfo::default().command_buffers(&[cmd]);
            // vk_check!(unsafe { self.device.queue_submit(self.queue, &[submit_info], vk::Fence::null()) });
        }

        // TODO: Wait for queue idle
        // vk_check!(unsafe { self.device.queue_wait_idle(self.queue) });

        // TODO: Reset command pools
        // self.cmd_manager.reset_all();

        // Clear tasks for the next frame
        self.tasks.clear();
        self.subgraphs.clear();

        Ok(())
    }
}
