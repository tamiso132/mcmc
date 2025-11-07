//! Translation of 64node.hpp/cpp
//!
//! This module provides low-level bitstream utilities for manipulating
//! the 65-bit SVO node data. It is an internal implementation detail.

/// Corresponds to the C++ UnsafeBitstream64.
///
/// This struct provides methods to read and write 64-bit values
/// from a u64 slice at *unaligned* bit offsets.
pub(super) struct Bitstream64<'a> {
    data: &'a mut [u64],
}

impl<'a> Bitstream64<'a> {
    pub(super) fn new(data: &'a mut [u64]) -> Self {
        Self { data }
    }

    /// Corresponds to `read64`.
    pub(super) fn read64(&self, bit_offset: u32) -> u64 {
        let index = (bit_offset >> 6) as usize; // bit_offset / 64
        let shift = bit_offset & 63; // bit_offset % 64

        let lo = self.data[index];
        let hi = self.data.get(index + 1).copied().unwrap_or(0);

        if shift == 0 {
            return lo;
        }
        let safe_shift = (64u32 - shift) & 63u32;
        let test = (1u64.wrapping_shl(shift)).wrapping_sub(1);
        (lo >> shift) | ((hi & test).wrapping_shl(safe_shift))
    }

    /// Corresponds to `write64_masked`.
    pub(super) fn write64_masked(&mut self, bit_offset: u32, value: u64, mask: u64) {
        let index = (bit_offset >> 6) as usize;
        let shift = bit_offset & 63;
        let safe_shift = (64u32 - shift) & 63u32;

        let mask_write_low = mask.wrapping_shl(shift);
        let mask_write_high = mask.wrapping_shr(safe_shift);
        let value_write_low = value.wrapping_shl(shift);
        let value_write_high = value.wrapping_shr(safe_shift);

        self.data[index] = (value_write_low) | (self.data[index] & !mask_write_low);
        if let Some(hi) = self.data.get_mut(index + 1) {
            *hi = (value_write_high) | (*hi & !mask_write_high);
        }
    }

    /// Corresponds to `write64_set`
    pub(super) fn write64_set(&mut self, bit_offset: u32, value: u64) {
        self.write64_masked(bit_offset, value, value);
    }

    /// Corresponds to `write64_unset`
    pub(super) fn write64_unset(&mut self, bit_offset: u32, value: u64) {
        self.write64_masked(bit_offset, 0, value);
    }
}

/// Corresponds to `test_function` from 64node.cpp.
/// `pub(crate)` makes it visible to `RaytraceWorld` in `mod.rs`.
pub(crate) fn test_function() {
    let mut test_data: Vec<u64> = vec![0; 500];
    {
        let mut bit_stream = Bitstream64::new(&mut test_data);
        // ... (rest of the test logic from the previous file) ...
    }
    log::info!("Bitstream64 test passed!");
}

/// Corresponds to `nodeutil` namespace.
/// These are now free functions within the `bitstream` module.
pub(super) mod node_util {
    use glam::IVec3;

    use super::Bitstream64;

    pub(super) const NODE_TOTAL_BITS: u32 = 65;
    pub(super) const OCCUPANCY_OFFSET: u32 = 1;
    pub(super) const LEAF_MASK: u64 = 1;

    #[inline(always)]
    pub(crate) fn get_local_index(x: u32, y: u32, z: u32) -> u32 {
        let child_bit = x + (y << 2) + (z << 4);
        assert!(child_bit < 64);
        child_bit
    }

    #[inline(always)]
    pub(crate) fn get_child_pos_from_local(child_bit: u32) -> IVec3 {
        assert!(child_bit < 64);
        let x = child_bit & 0b11;
        let y = (child_bit >> 2) & 0b11;
        let z = (child_bit >> 4) & 0b11;
        IVec3::new(x as i32, y as i32, z as i32)
    }

    #[inline(always)]
    pub(crate) fn is_leaf(nodes: &mut [u64], array_index: u32) -> bool {
        let tot_bit_offset = array_index * NODE_TOTAL_BITS;
        let mut stream = Bitstream64::new(nodes);
        (stream.read64(tot_bit_offset) & LEAF_MASK) != 0
    }

    #[inline(always)]
    pub(crate) fn get_occupancy_mask(nodes: &mut [u64], array_index: u32) -> u64 {
        let parent_bit_index = array_index * NODE_TOTAL_BITS + OCCUPANCY_OFFSET;
        let mut stream = Bitstream64::new(nodes);
        stream.read64(parent_bit_index)
    }

    #[inline(always)]
    pub(crate) fn set_leaf(nodes: &mut [u64], array_index: u32) {
        let array_total_bits = array_index * NODE_TOTAL_BITS;
        let mut stream = Bitstream64::new(nodes);
        stream.write64_set(array_total_bits, LEAF_MASK);
    }

    #[inline(always)]
    pub(super) fn unset_leaf(nodes: &mut [u64], array_index: u32) {
        let array_total_bits = array_index * NODE_TOTAL_BITS;
        let mut stream = Bitstream64::new(nodes);
        stream.write64_unset(array_total_bits, LEAF_MASK);
    }

    #[inline(always)]
    pub(crate) fn set_occupancy_bit(nodes: &mut [u64], array_index: u32, local: IVec3) {
        let relative_offset = get_local_index(local.x as u32, local.y as u32, local.z as u32);
        let array_total_bits = array_index * NODE_TOTAL_BITS;
        let mut stream = Bitstream64::new(nodes);
        stream.write64_set(array_total_bits + OCCUPANCY_OFFSET, 1u64 << relative_offset);
    }

    #[inline(always)]
    pub(crate) fn set_occupancy_mask(nodes: &mut [u64], array_index: u32, occupancy_mask: u64) {
        let array_total_bits = array_index * NODE_TOTAL_BITS;
        let mut stream = Bitstream64::new(nodes);
        stream.write64_set(array_total_bits + OCCUPANCY_OFFSET, occupancy_mask);
    }
}
