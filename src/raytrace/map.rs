//! Translation of map.hpp
//!
//! This module contains the `Tree64Builder`, which is responsible for
//! constructing the SVO on the CPU before it's uploaded to the GPU.

use super::node::{Bitstream64, node_util};
use glam::{IVec3, Vec3};
use std::collections::{HashMap, VecDeque};

// This struct is from `rayshared.inl`.
// It's the key for the node hashmap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct NodeKey {
    // We use the u32 bit-representation of f32s to make this
    // struct hashable and equatable.
    pub min_corner_x: u32,
    pub min_corner_y: u32,
    pub min_corner_z: u32,
    pub level: u32,
}

// Custom Hash implementation for f32 keys
impl std::hash::Hash for NodeKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.min_corner_x.hash(state);
        self.min_corner_y.hash(state);
        self.min_corner_z.hash(state);
        self.level.hash(state);
    }
}

impl NodeKey {
    pub fn new(min_corner: Vec3, level: u32) -> Self {
        Self {
            min_corner_x: min_corner.x.to_bits(),
            min_corner_y: min_corner.y.to_bits(),
            min_corner_z: min_corner.z.to_bits(),
            level,
        }
    }

    pub fn min_corner(&self) -> Vec3 {
        Vec3::new(
            f32::from_bits(self.min_corner_x),
            f32::from_bits(self.min_corner_y),
            f32::from_bits(self.min_corner_z),
        )
    }
}

// --- From map.hpp ---

#[derive(Debug, Clone, Copy)]
struct ChildIndex {
    local_index: u32,
    array_index: u32,
}

/// Corresponds to `Tree64Builder::NodeStorage`
#[derive(Debug, Clone)]
struct NodeStorage {
    /// Replaces `m_array_index` (GPUHashMap)
    array_index_map: HashMap<NodeKey, u32>,

    /// Corresponds to `m_nodes` (std::vector<Node64>)
    /// This is the raw bitstream data.
    nodes: Vec<u64>,

    /// Corresponds to `m_children`
    /// This is a temporary structure for the unsorted tree.
    children: Vec<Vec<ChildIndex>>,

    /// Corresponds to `m_node_len`
    node_len: u32,
}

impl NodeStorage {
    /// C++ constructor `NodeStorage(u32 resize_count)`
    pub fn with_capacity(capacity: usize) -> Self {
        let mut nodes = Vec::new();
        // Reserve space, *2 for the 65-bit padding logic
        nodes.reserve(capacity.max(1) * 2);
        nodes.push(0); // Add the first node (C++: `m_nodes.push_back({0});`)

        Self {
            array_index_map: HashMap::with_capacity(capacity),
            nodes,
            children: Vec::with_capacity(capacity),
            node_len: 0,
        }
    }

    /// Corresponds to `insert_node`
    fn insert_node(&mut self, key: NodeKey) -> u32 {
        assert!(!self.array_index_map.contains_key(&key));

        let new_index = self.node_len;
        self.array_index_map.insert(key, new_index);
        self.children.push(Vec::new()); // Add empty child list

        // This logic matches the C++ padding logic
        // to ensure bitstream read64's `data[index+1]` is safe.
        let nodes_64 = self.nodes.len() >> 6;
        self.nodes.push(0);
        let nodes_64_post = self.nodes.len() >> 6;
        if nodes_64 != nodes_64_post {
            self.nodes.push(0);
        }

        self.node_len += 1;
        new_index
    }

    /// Corresponds to `insert_node_no_key` (used by sort)
    fn insert_node_no_key(&mut self) -> u32 {
        let new_index = self.node_len;
        // Need to add an empty child list to keep indices in sync
        self.children.push(Vec::new());

        let nodes_64 = self.nodes.len() >> 6;
        self.nodes.push(0);
        let nodes_64_post = self.nodes.len() >> 6;
        if nodes_64 != nodes_64_post {
            self.nodes.push(0);
        }

        self.node_len += 1;
        new_index
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

        // Find insert position
        match parent_children.binary_search_by_key(&local_index, |ci| ci.local_index) {
            Ok(_) => {
                // This shouldn't happen based on C++ assert
                panic!("Child already exists at local_index {}", local_index);
            }
            Err(i) => {
                // Insert at sorted position
                parent_children.insert(i, child_insert);
            }
        }
    }

    /// Corresponds to `get_child_index_from_child_list`
    fn get_child_index_from_child_list(&self, parent_index: u32, child_number: u32) -> u32 {
        self.children[parent_index as usize][child_number as usize].array_index
    }

    /// Corresponds to `add_if_no_exist`
    fn add_if_no_exist(&mut self, key: NodeKey) -> u32 {
        // Use `entry` for an efficient "get or insert"
        match self.array_index_map.get(&key) {
            Some(&index) => index,
            None => self.insert_node(key),
        }
    }

    /// Corresponds to `add_unchecked`
    fn add_unchecked(&mut self, key: NodeKey) -> u32 {
        self.insert_node(key)
    }

    // --- Bitstream wrapper functions ---

    fn set_occupancy_mask(&mut self, array_index: u32, occupancy_mask: u64) {
        node_util::set_occupancy_mask(&mut self.nodes, array_index, occupancy_mask);
    }

    fn set_occupancy_mask_bit(&mut self, array_index: u32, local: IVec3) {
        node_util::set_occupancy_bit(&mut self.nodes, array_index, local);
    }

    fn set_leaf(&mut self, array_index: u32) {
        node_util::set_leaf(&mut self.nodes, array_index);
    }

    fn is_leaf(&mut self, array_index: u32) -> bool {
        node_util::is_leaf(&mut self.nodes, array_index)
    }

    fn get_array_index(&self, key: NodeKey) -> Option<u32> {
        self.array_index_map.get(&key).copied()
    }

    fn get_occupancy_mask(&mut self, array_index: u32) -> u64 {
        node_util::get_occupancy_mask(&mut self.nodes, array_index)
    }

    pub fn get_nodes_len(&self) -> u32 {
        self.node_len
    }

    pub fn get_nodes_slice(&self) -> &[u64] {
        &self.nodes
    }

    pub fn print_profile(&self) {
        log::info!(
            "Hashmap Capacity: {}, Hashmap Len: {}, Nodes Len: {}, Nodes Vec Capacity: {}",
            self.array_index_map.capacity(),
            self.array_index_map.len(),
            self.node_len,
            self.nodes.capacity()
        );
    }
}

/// Corresponds to `Tree64Builder`
pub struct Tree64Builder {
    nodes: NodeStorage,
    max_level: u32,
    root_size: f32,
    root_min: Vec3,
}

// C++ `constexpr static`
const LOWEST_UNIT_VOXEL: f32 = 1.0;
const LOWEST_UNIT_SCALE: f32 = LOWEST_UNIT_VOXEL / 4.0;

impl Tree64Builder {
    /// Corresponds to `Tree64Builder(int max_level, glm::vec3 center)`
    pub fn new(max_level: u32, center: Vec3) -> Self {
        let root_size = Self::get_child_size(max_level) * 4.0;
        let root_min = center - (root_size / 2.0);

        let key = NodeKey::new(root_min, max_level);

        let mut nodes = NodeStorage::with_capacity(1024); // Start with some capacity
        nodes.insert_node(key);

        Self {
            nodes,
            max_level,
            root_size,
            root_min,
        }
    }

    /// Corresponds to `insert_box`
    pub fn insert_box(&mut self, min_corner: Vec3, max_corner: Vec3) {
        let child_size = self.root_size / 4.0;
        let node_min_corner = self.root_min;
        let node_max_corner = node_min_corner + self.root_size;

        let intersection_min = min_corner.max(node_min_corner);
        let intersection_max = max_corner.min(node_max_corner);

        let root_key = NodeKey::new(self.root_min, self.max_level);

        if intersection_min.x >= intersection_max.x
            || intersection_min.y >= intersection_max.y
            || intersection_min.z >= intersection_max.z
        {
            log::warn!("Box is completely outside the tree bounds.");
            return;
        }

        let mut count = 0;
        // Replicate the C++ loop logic
        let mut y = intersection_min.y;
        while y < intersection_max.y {
            let mut z = intersection_min.z;
            while z < intersection_max.z {
                let mut x = intersection_min.x;
                while x < intersection_max.x {
                    count += 1;
                    let pos = Vec3::new(x, y, z);
                    self.insert_recursive(root_key, child_size, pos);
                    x += LOWEST_UNIT_VOXEL;
                }
                z += LOWEST_UNIT_VOXEL;
            }
            y += LOWEST_UNIT_VOXEL;
        }
        log::info!("Inserted {} voxels for box.", count);
    }

    /// Corresponds to `insert_recursive`
    fn insert_recursive(&mut self, mut parent: NodeKey, mut child_size: f32, pos: Vec3) {
        // This struct replaces the C++ `std::queue<NodeStack>`
        struct NodeStackItem {
            local_index: u32,
            array_index: u32,
            is_new_insert: bool,
        }
        // C++ used `std::queue`, which is FIFO.
        let mut stack: VecDeque<NodeStackItem> = VecDeque::new();

        while parent.level > 0 {
            let local_pos_f = (pos - parent.min_corner()) / child_size;
            let local_pos = local_pos_f.floor().as_ivec3();

            let array_index = self.nodes.add_if_no_exist(parent);

            // if leaf, means that everything is occupied
            if self.nodes.is_leaf(array_index) {
                // This node is already full, so we can't insert deeper.
                return;
            }

            let occupancy_mask_before = self.nodes.get_occupancy_mask(array_index);
            self.nodes.set_occupancy_mask_bit(array_index, local_pos);
            let occupancy_mask_after = self.nodes.get_occupancy_mask(array_index);

            let local_index = node_util::get_local_index(
                local_pos.x as u32,
                local_pos.y as u32,
                local_pos.z as u32,
            );

            // C++: `if (parent.level < m_max_level)`
            if parent.level != self.max_level {
                let parent_stack = stack.pop_front().expect("Stack should not be empty");
                if parent_stack.is_new_insert {
                    self.nodes.update_parent_child_list(
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

            // Just became a leaf
            if occupancy_mask_after == u64::MAX {
                self.nodes.set_leaf(array_index);
                return;
            }

            let new_min_corner = parent.min_corner() + (local_pos.as_vec3()) * child_size;
            parent = NodeKey::new(new_min_corner, parent.level - 1);
            child_size /= 4.0;
        }

        // adding leaf (level == 0)
        let leaf_index = self.nodes.add_unchecked(parent);
        self.nodes.set_leaf(leaf_index);

        let parent_stack = stack
            .pop_front()
            .expect("Stack should not be empty on final step");
        if parent_stack.is_new_insert {
            self.nodes.update_parent_child_list(
                parent_stack.array_index,
                parent_stack.local_index,
                leaf_index,
            );
        } else {
            // C++: abort()
            panic!("Leaf node was somehow already inserted?");
        }
    }

    /// Corresponds to `sort`
    pub fn sort(&mut self) {
        log::debug!("SORTING BEGIN");

        struct QueueStackItem {
            key: NodeKey,
            old_array_index: u32,
            parent_index: u32,
            local_index: u32,
            is_key_insert: bool,
        }

        let mut sorted_nodes = NodeStorage::with_capacity(self.nodes.get_nodes_len() as usize);
        let mut queue: VecDeque<QueueStackItem> = VecDeque::new();

        queue.push_back(QueueStackItem {
            key: NodeKey::new(self.root_min, self.max_level),
            old_array_index: 0,
            parent_index: 0,
            local_index: 0,
            is_key_insert: true,
        });

        while let Some(queue_val) = queue.pop_front() {
            let node_key = queue_val.key;
            let array_index = queue_val.old_array_index;
            let parent_index = queue_val.parent_index;
            let local_index = queue_val.local_index;
            let is_key_insert = queue_val.is_key_insert;

            let occupancy_mask = self.nodes.get_occupancy_mask(array_index);

            assert_eq!(
                self.nodes.get_array_index(node_key),
                Some(array_index),
                "Node key mismatch during sort"
            );

            let sort_index = if is_key_insert {
                sorted_nodes.insert_node(node_key)
            } else {
                sorted_nodes.insert_node_no_key()
            };

            // if not root
            if node_key.level != self.max_level {
                sorted_nodes.update_parent_child_list(parent_index, local_index, sort_index);
            }

            sorted_nodes.set_occupancy_mask(sort_index, occupancy_mask);

            assert_eq!(
                sorted_nodes.get_occupancy_mask(sort_index),
                occupancy_mask,
                "Occupancy mask read/write failed"
            );

            let child_size = Self::get_child_size(node_key.level);

            if self.nodes.is_leaf(array_index) {
                sorted_nodes.set_leaf(sort_index);
                continue;
            }

            if occupancy_mask == u64::MAX {
                // This was a full node, but not lowest level.
                // In C++ this is a leaf.
                sorted_nodes.set_leaf(sort_index);
                log::warn!("Full node (u64::MAX) found at level > 0. Treating as leaf.");
                continue;
            }

            // should not be trying to find children for the lowest level
            assert_ne!(node_key.level, 0, "Non-leaf node at level 0");

            let mut count = 0;
            let mut occupancy_mask_iter = occupancy_mask;

            while occupancy_mask_iter != 0 {
                // C++: __builtin_ctzll
                let local_index = occupancy_mask_iter.trailing_zeros();
                // C++: occupancy_mask &= occupancy_mask - 1;
                occupancy_mask_iter &= occupancy_mask_iter - 1; // clear lowest bit

                let child_pos = node_util::get_child_pos_from_local(local_index).as_vec3();
                let child_index = self
                    .nodes
                    .get_child_index_from_child_list(array_index, count);

                assert!(
                    array_index < child_index,
                    "Child index should be greater than parent"
                );

                let child_key = NodeKey::new(
                    node_key.min_corner() + child_pos * child_size,
                    node_key.level - 1,
                );

                queue.push_back(QueueStackItem {
                    key: child_key,
                    old_array_index: child_index,
                    parent_index: sort_index,
                    local_index,
                    is_key_insert: count == 0, // C++ `count == 0` logic
                });
                count += 1;
            }
        }

        log::info!("Sort complete.");
        self.nodes = sorted_nodes;
    }

    /// Corresponds to `get_child_size`
    #[inline(always)]
    fn get_child_size(level: u32) -> f32 {
        // C++: LOWEST_UNIT_SCALE * f32(1u << (level * 2))
        LOWEST_UNIT_SCALE * (1u64 << (level * 2)) as f32
    }

    pub fn print_profile(&self) {
        self.nodes.print_profile();
    }

    /// Provides access to the final, sorted node data for GPU upload.
    pub fn get_node_data(&self) -> &[u64] {
        self.nodes.get_nodes_slice()
    }
}
