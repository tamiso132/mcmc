//! RenderGraph Module
//!
//! This module contains a translation of the C++ render graph system.
//!
//! Public-facing types:
//! - `RenderGraph`: The main graph object, owned by `VkLibrary`.
//! - `TaskBuilder`: Used to construct a `Task` with type-safe resource handles.
//! - `Access`: Bitflag enum for resource access types (Read, Write).
//! - `TaskType`: Enum for task types (Graphics, Compute, Transfer).
//! - `PassEncoder`: A wrapper for command buffers passed into a Task's execute function.

// Expose the sub-modules
mod configurator;
mod graph;
mod task;
mod types;
mod util;

// Re-export the public-facing types for easy use
pub use configurator::RenderGraphConfigurator;
pub use graph::RenderGraph;
pub use task::{Task, TaskBuilder};
pub use types::{Access, PassEncoder, TaskType};