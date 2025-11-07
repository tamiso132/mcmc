//! A direct Rust port of the C++ `GPUHashMap`.
//!
//! This is a linear-probing hashmap with a power-of-two capacity.
//! Its internal `table: Vec<HashEntry>` is intended to be uploaded
//! directly to the GPU.

use glam::Vec3;
use std::hash::{Hash, Hasher};

// --- Structs (from shader_struct.hpp / rayshared.inl) ---

/// Corresponds to `shader::NodeKey`.
///
/// We use `glam::Vec3` for the vector, but implement `PartialEq`, `Eq`,
/// and `Hash` using the `f32::to_bits()` representation to match
/// the C++ `std::bit_cast` hashing and achieve bit-wise equality.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub(super) struct NodeKey {
    pub min_corner: Vec3,
    pub level: u32,
}

impl PartialEq for NodeKey {
    fn eq(&self, other: &Self) -> bool {
        self.level == other.level
            && self.min_corner.x.to_bits() == other.min_corner.x.to_bits()
            && self.min_corner.y.to_bits() == other.min_corner.y.to_bits()
            && self.min_corner.z.to_bits() == other.min_corner.z.to_bits()
    }
}
impl Eq for NodeKey {}

impl Hash for NodeKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // This hashing must be consistent with the `hash::fnv1a` function.
        // We use the `f32::to_bits()` representation.
        self.min_corner.x.to_bits().hash(state);
        self.min_corner.y.to_bits().hash(state);
        self.min_corner.z.to_bits().hash(state);
        self.level.hash(state);
    }
}

/// Corresponds to `shader::HashEntry`
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub(super) struct HashEntry {
    pub key: NodeKey,
    pub value: u32,
}

// --- Sentinel Keys ---

/// Corresponds to `EMPTY_KEY`
pub(super) const EMPTY_KEY: NodeKey = NodeKey {
    min_corner: Vec3::new(-1.0, -1.0, -1.0),
    level: 0xFFFFFFFE,
};

/// Corresponds to `REMOVED_KEY`
pub(super) const REMOVED_KEY: NodeKey = NodeKey {
    min_corner: Vec3::new(-2.0, -2.0, -2.0),
    level: 0xFFFFFFFD,
};

impl Default for HashEntry {
    fn default() -> Self {
        Self {
            key: EMPTY_KEY,
            value: 0,
        }
    }
}

// --- Hashing Functions (Port of `Hash` struct) ---

mod hash {
    use super::NodeKey;

    #[inline(always)]
    fn scramble(v: u32) -> u32 {
        let mut v = v;
        v ^= v >> 16;
        v = v.wrapping_mul(2246822507);
        v ^= v >> 13;
        v = v.wrapping_mul(3266489909);
        v ^= v >> 16;
        v
    }

    /// Corresponds to `Hash::fnv1a`
    #[inline(always)]
    pub(super) fn fnv1a(key: &NodeKey) -> u32 {
        let mut h = 2166136261u32;
        h ^= scramble(key.min_corner.x.to_bits());
        h = h.wrapping_mul(16777619);
        h ^= scramble(key.min_corner.y.to_bits());
        h = h.wrapping_mul(16777619);
        h ^= scramble(key.min_corner.z.to_bits());
        h = h.wrapping_mul(16777619);
        h ^= key.level;
        h = h.wrapping_mul(16777619);
        h
    }
}

// --- GPUHashMap ---

/// Corresponds to `GPUHashMap`
#[derive(Debug, Clone)]
pub struct GpuHashmap {
    pub table: Vec<HashEntry>,
    pub max_collision: u32,
    pub len: u32,
}

impl GpuHashmap {
    /// C++: `explicit GPUHashMap(size_t capacity)`
    /// `capacity` in C++ is the log2 of the size.
    pub fn new(log2_capacity: u32) -> Self {
        let capacity = 1 << log2_capacity;
        Self {
            // C++: `table(1ull << capacity, {EMPTY_KEY, 0})`
            table: vec![HashEntry::default(); capacity],
            max_collision: 0,
            len: 0,
        }
    }

    pub fn insert(&mut self, key: &NodeKey, value: u32) {
        let capacity_mask = self.table.len() as u32 - 1;
        let index = hash::fnv1a(key) & capacity_mask;

        for i in 0..self.table.len() as u32 {
            let probe = ((i + index) & capacity_mask) as usize;

            if self.table[probe].key == EMPTY_KEY || self.table[probe].key == REMOVED_KEY {
                if self.max_collision > i {
                    self.max_collision = i;
                }
                if self.len > (self.table.len() as f32 * 0.7) as u32 {
                    self.resize();
                    return self.insert(key, value);
                }
                self.table[probe] = HashEntry { key: *key, value };
                self.len += 1;
                return;
            }
        }

        panic!();
    }

    fn capacity(&self) -> u32 {
        self.table.len() as u32
    }

    fn mask(&self) -> u32 {
        self.capacity() - 1
    }

    /// Corresponds to `get(key)` and `operator[](key)`
    /// This is a "find or insert" operation that returns
    /// a mutable reference to the value.
    pub(super) fn get_mut(&mut self, key: NodeKey) -> &mut u32 {
        let capacity = self.capacity();
        let mask = self.mask();
        let index = hash::fnv1a(&key) & mask;

        for i in 0..capacity {
            let probe = (index + i) & mask;

            if self.table[probe as usize].key != EMPTY_KEY {
                if self.table[probe as usize].key == key {
                    // Found it
                    return &mut self.table[probe as usize].value;
                }
            } else {
                // Found empty slot, insert here
                if self.max_collision > i {
                    self.max_collision = i;
                }
                self.table[probe as usize].key = key;
                self.table[probe as usize].value = 0; // Default value, will be overwritten

                self.len += 1;

                if self.len > (capacity * 7 / 10) {
                    // 0.7 load factor
                    self.resize();
                    // After resize, we must re-run `get_mut`
                    return self.get_mut(key);
                }
                return &mut self.table[probe as usize].value;
            }
        }

        panic!("GPUHashMap is full!");
    }

    /// Corresponds to `contains(key)`
    pub(super) fn contains(&self, key: &NodeKey) -> bool {
        let capacity = self.capacity();
        let mask = self.mask();
        let index = hash::fnv1a(key) & mask;

        for i in 0..capacity {
            let probe = (i + index) & mask;
            let entry = &self.table[probe as usize];

            if entry.key != REMOVED_KEY {
                if entry.key == *key {
                    return true;
                }
            }
            if entry.key == EMPTY_KEY {
                return false;
            }
        }
        false
    }

    /// Corresponds to `resize()`
    fn resize(&mut self) {
        let old_capacity = self.capacity();
        let new_capacity = old_capacity << 1;
        let new_mask = new_capacity - 1;

        let mut new_table = vec![HashEntry::default(); new_capacity as usize];

        // Re-hash all existing elements
        for i in 0..old_capacity {
            let entry = &self.table[i as usize];
            if entry.key != EMPTY_KEY && entry.key != REMOVED_KEY {
                let index = hash::fnv1a(&entry.key) & new_mask;
                for ii in 0..new_capacity {
                    let n_index = (index + ii) & new_mask;
                    if new_table[n_index as usize].key == EMPTY_KEY {
                        new_table[n_index as usize] = *entry;
                        break;
                    }
                }
            }
        }

        self.table = new_table;
    }

    /// Corresponds to `get_table_slice()`
    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(
                self.table.as_ptr() as *const u8,
                self.table.len() * std::mem::size_of::<HashEntry>(),
            )
        }
    }
}
