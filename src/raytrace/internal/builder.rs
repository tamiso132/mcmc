//! Translation of map.hpp
//!
//! This module contains the internal "unsorted" SVO storage and the
//! `sort` function that linearizes the tree for the GPU.

use crate::raytrace::internal::hashmap::{GpuHashmap, NodeKey};

use super::bitstream::node_util;
use glam::Vec3;
use std::collections::{HashMap, VecDeque};

// --- C++ `rayshared.inl` Struct ---
// This is the key for the node hashmap.

impl NodeKey {
    pub(crate) fn new(min_corner: Vec3, level: u32) -> Self {
        Self { min_corner, level }
    }
}

// --- C++ `Tree64Builder::NodeStorage::ChildIndex` ---
#[derive(Debug, Clone, Copy)]
struct ChildIndex {
    local_index: u32,
    array_index: u32,
}

/// Placeholder for the GPU-side hashmap.
///
/// **IMPORTANT:** The C++ code uses a custom `GPUHashMap` from `structure/hashmap.hpp`.
/// This is a linear-probed hashmap whose internal `table` is uploaded directly
/// to the GPU.
///
/// This `std::collections::HashMap` is **NOT** compatible.
/// You will need to port the C++ `GPUHashMap` here and store its table
/// in the final `SvoTree`.

/// Corresponds to the C++ `NodeStorage`, but refactored to be an
/// internal, encapsulated struct. This version is for the *unsorted* tree.
#[derive(Debug, Clone)]
pub(crate) struct UnsortedStorage {
    /// The temporary hashmap for finding nodes by key during build.
    map: HashMap<NodeKey, u32>,
    /// The bitstream data.
    pub(super) nodes: Vec<u64>,
    /// Temporary child lists used only for sorting.
    children: Vec<Vec<ChildIndex>>,
    /// Total number of nodes.
    node_len: u32,

    // --- Tree metadata (cached here for `insert_recursive`) ---
    max_level: u32,
    root_min: Vec3,
    root_key: NodeKey,
    child_size: f32,
}

/// A simpler storage used by the `sort` function.
#[derive(Debug, Clone)]
pub struct SortedStorage {
    pub(crate) nodes: Vec<u64>,
    node_len: u32,
}

// C++ `constexpr static`
const LOWEST_UNIT_VOXEL: f32 = 1.0;
const LOWEST_UNIT_SCALE: f32 = LOWEST_UNIT_VOXEL / 4.0;

#[inline(always)]
pub(crate) fn get_child_size(level: u32) -> f32 {
    LOWEST_UNIT_SCALE * (1u64 << (level * 2)) as f32
}

// --- `UnsortedStorage` (The Builder's internal state) ---
impl UnsortedStorage {
    /// Creates the storage and inserts the root node.
    pub fn new(root_min: Vec3, max_level: u32) -> Self {
        let mut nodes = Vec::new();
        nodes.reserve(1024 * 2); // Start with some capacity
        nodes.push(0); // Root node data

        let root_key = NodeKey::new(root_min, max_level);
        let mut map = HashMap::with_capacity(1024);
        map.insert(root_key, 0); // Root is at index 0

        Self {
            map,
            nodes,
            children: vec![Vec::new()], // Child list for root node
            node_len: 1,
            max_level,
            root_min,
            root_key,
            child_size: get_child_size(max_level) / 4.0, // Pre-calc first child size
        }
    }

    /// Corresponds to `insert_node`
    fn insert_node(&mut self, key: NodeKey) -> u32 {
        let new_index = self.node_len;
        self.map.insert(key, new_index);
        self.children.push(Vec::new());

        // Padding logic from C++
        let nodes_64 = self.nodes.len() >> 6;
        self.nodes.push(0);
        let nodes_64_post = self.nodes.len() >> 6;
        if nodes_64 != nodes_64_post {
            self.nodes.push(0);
        }

        self.node_len += 1;
        new_index
    }

    /// Corresponds to `add_if_no_exist`
    fn add_if_no_exist(&mut self, key: NodeKey) -> u32 {
        match self.map.get(&key) {
            Some(&index) => index,
            None => self.insert_node(key),
        }
    }

    /// Corresponds to `add_unchecked` (for level 0 leaves)
    fn add_unchecked(&mut self, key: NodeKey) -> u32 {
        self.insert_node(key)
    }

    /// Corresponds to `update_parent_child_list`
    fn update_parent_child_list(
        &mut self,
        parent_array_index: u32,
        local_index: u32,
        child_array_index: u32,
    ) {
        let child_insert = ChildIndex {
            local_index,
            array_index: child_array_index,
        };
        let parent_children = &mut self.children[parent_array_index as usize];
        match parent_children.binary_search_by_key(&local_index, |ci| ci.local_index) {
            Ok(_) => panic!("Child already exists at local_index {}", local_index),
            Err(i) => parent_children.insert(i, child_insert),
        }
    }

    /// Corresponds to `insert_recursive`
    pub fn insert_recursive(&mut self, pos: Vec3) {
        let mut parent_key = self.root_key;
        let mut child_size = self.child_size;

        struct NodeStackItem {
            local_index: u32,
            array_index: u32,
            is_new_insert: bool,
        }
        let mut stack: VecDeque<NodeStackItem> = VecDeque::new();

        while parent_key.level > 0 {
            let local_pos_f = (pos - parent_key.min_corner) / child_size;
            let local_pos = local_pos_f.floor().as_ivec3();

            let array_index = self.add_if_no_exist(parent_key);

            if node_util::is_leaf(&mut self.nodes, array_index) {
                return; // Node is already full
            }

            let occupancy_mask_before = node_util::get_occupancy_mask(&mut self.nodes, array_index);
            node_util::set_occupancy_bit(&mut self.nodes, array_index, local_pos);
            let occupancy_mask_after = node_util::get_occupancy_mask(&mut self.nodes, array_index);

            let local_index = node_util::get_local_index(
                local_pos.x as u32,
                local_pos.y as u32,
                local_pos.z as u32,
            );

            if parent_key.level != self.max_level {
                let parent_stack = stack.pop_front().expect("Stack should not be empty");
                if parent_stack.is_new_insert {
                    self.update_parent_child_list(
                        parent_stack.array_index,
                        parent_stack.local_index,
                        array_index,
                    );
                }
            }

            stack.push_back(NodeStackItem {
                local_index,
                array_index,
                is_new_insert: occupancy_mask_after != occupancy_mask_before,
            });

            if occupancy_mask_after == u64::MAX {
                node_util::set_leaf(&mut self.nodes, array_index);
                return;
            }

            let new_min_corner = parent_key.min_corner + (local_pos.as_vec3()) * child_size;
            parent_key = NodeKey::new(new_min_corner, parent_key.level - 1);
            child_size /= 4.0;
        }

        // --- Add level 0 leaf ---
        let leaf_index = self.add_unchecked(parent_key);
        node_util::set_leaf(&mut self.nodes, leaf_index);

        let parent_stack = stack.pop_front().expect("Stack should not be empty");
        if parent_stack.is_new_insert {
            self.update_parent_child_list(
                parent_stack.array_index,
                parent_stack.local_index,
                leaf_index,
            );
        } else {
            panic!("Leaf node was somehow already inserted?");
        }
    }

    pub fn print_profile(&self, name: &str) {
        log::info!(
            "SVO Profile ({}): Map Len: {}, Nodes Len: {}, Nodes Vec Cap: {}",
            name,
            self.map.len(),
            self.node_len,
            self.nodes.capacity()
        );
    }
}

// --- `SortedStorage` (The `SvoTree`'s internal data) ---
impl SortedStorage {
    fn with_capacity(capacity: usize) -> Self {
        let mut nodes = Vec::with_capacity(capacity * 2);
        nodes.push(0); // Root node
        Self { nodes, node_len: 0 }
    }

    /// Corresponds to `insert_node_no_key`
    fn insert_node(&mut self) -> u32 {
        let new_index = self.node_len;

        let nodes_64 = self.nodes.len() >> 6;
        self.nodes.push(0);
        let nodes_64_post = self.nodes.len() >> 6;
        if nodes_64 != nodes_64_post {
            self.nodes.push(0);
        }

        self.node_len += 1;
        new_index
    }

    pub fn print_profile(&self, name: &str) {
        log::info!(
            "SVO Profile ({}): Nodes Len: {}, Nodes Vec Cap: {}",
            name,
            self.node_len,
            self.nodes.capacity()
        );
    }
}

/// Corresponds to the C++ `sort` function.
/// This is the core of the `SvoBuilder::build()` process.
pub(crate) fn sort(mut unsorted: UnsortedStorage) -> (SortedStorage, GpuHashmap) {
    struct QueueStackItem {
        key: NodeKey,
        old_array_index: u32,
        parent_index: u32, // Parent's index in the *new* sorted list
        local_index: u32,
    }

    let mut sorted_storage = SortedStorage::with_capacity(unsorted.node_len as usize);

    // This is the *final* hashmap that will be uploaded to the GPU.
    // **TODO**: This MUST be replaced with a port of the C++ `GPUHashMap`.
    let mut sorted_hashmap: GpuHashmap = GpuHashmap::new(unsorted.node_len);

    let mut queue: VecDeque<QueueStackItem> = VecDeque::new();

    queue.push_back(QueueStackItem {
        key: unsorted.root_key,
        old_array_index: 0,
        parent_index: 0,
        local_index: 0,
    });

    while let Some(queue_val) = queue.pop_front() {
        let node_key = queue_val.key;
        let old_array_index = queue_val.old_array_index;

        // This is the index in the *new* sorted list
        let sort_index = sorted_storage.insert_node();

        // Populate the final hashmap
        sorted_hashmap.insert(&node_key, sort_index);

        // This part is different from C++. The `update_parent_child_list`
        // is not needed, as the new parent-child relationship is implicit
        // by `popcount` on the occupancy mask (a linear octree).

        let occupancy_mask = node_util::get_occupancy_mask(&mut unsorted.nodes, old_array_index);
        node_util::set_occupancy_mask(&mut sorted_storage.nodes, sort_index, occupancy_mask);

        let child_size = get_child_size(node_key.level);

        if node_util::is_leaf(&mut unsorted.nodes, old_array_index) || occupancy_mask == u64::MAX {
            node_util::set_leaf(&mut sorted_storage.nodes, sort_index);
            continue;
        }

        assert_ne!(node_key.level, 0, "Non-leaf node at level 0");

        let mut count = 0;
        let mut occupancy_mask_iter = occupancy_mask;

        while occupancy_mask_iter != 0 {
            let local_index = occupancy_mask_iter.trailing_zeros();
            occupancy_mask_iter &= occupancy_mask_iter - 1;

            let child_pos = node_util::get_child_pos_from_local(local_index).as_vec3();
            let old_child_index =
                unsorted.children[old_array_index as usize][count as usize].array_index;

            let child_key = NodeKey::new(
                node_key.min_corner + child_pos * child_size,
                node_key.level - 1,
            );

            queue.push_back(QueueStackItem {
                key: child_key,
                old_array_index: old_child_index,
                parent_index: sort_index,
                local_index,
            });
            count += 1;
        }
    }

    (sorted_storage, sorted_hashmap)
}
