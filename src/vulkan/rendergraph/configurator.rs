//! Translation of safety.hpp
//!
//! Defines the type-safe `RenderGraphConfigurator` which is the
//! *only* public way to import or replace resources in the graph.
//!
//! FIX: Removed lifetime 'a from RenderGraph to fix variance issue.

use crate::vulkan;

use super::graph::RenderGraph;
use crate::vulkan::resource::ResourceHandle;
use ash::vk;

/// Corresponds to `vklib::RenderGraphConfigurator<REnum>`
///
/// This is a short-lived struct created by `RenderGraph::configure()`
/// that provides a type-safe API for modifying the graph's resources.
/// It holds a mutable reference to the graph.
pub struct RenderGraphConfigurator<'graph, REnum>
where
    REnum: Into<u32> + Copy,
{
    pub(super) graph: &'graph mut RenderGraph, // <-- Removed 'graph lifetime from RenderGraph
    _phantom: std::marker::PhantomData<REnum>,
}

impl<'graph, REnum> RenderGraphConfigurator<'graph, REnum>
where
    REnum: Into<u32> + Copy,
{
    /// Creates a new configurator.
    /// This is only callable from within the `RenderGraph` module.
    pub(super) fn new(graph: &'graph mut RenderGraph) -> Self {
        // <-- Removed 'graph lifetime
        Self {
            graph,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Corresponds to `import_buffer`
    pub fn import_buffer(&mut self, index: REnum, handle: ResourceHandle) -> &mut Self {
        self.graph.import_buffer_internal(index.into(), handle);
        self
    }

    /// Corresponds to `import_image`
    pub fn import_image(
        &mut self,
        index: REnum,
        handle: ResourceHandle,
        init_layout: vk::ImageLayout,
    ) -> &mut Self {
        self.graph
            .import_image_internal(index.into(), handle, init_layout);
        self
    }

    /// Corresponds to `replace_resource`
    pub fn replace_resource(&mut self, index: REnum, new_res: ResourceHandle) -> &mut Self {
        self.graph.replace_resource_internal(index.into(), new_res);
        self
    }
}
