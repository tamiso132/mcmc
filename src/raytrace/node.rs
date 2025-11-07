//! Translation of 64node.hpp/cpp
//!
//! This module provides low-level bitstream utilities for manipulating
//! the 65-bit SVO node data.

use glam::IVec3;
use std::num::Wrapping;

// --- From 64node.hpp ---

/// Corresponds to the C++ UnsafeBitstream64.
///
/// This struct provides methods to read and write 64-bit values
/// from a u64 slice at *unaligned* bit offsets. Each "node" in
/// the C++ code is 65 bits (1-bit leaf + 64-bit occupancy).
pub struct Bitstream64<'a> {
    data: &'a mut [u64],
}

impl<'a> Bitstream64<'a> {
    pub fn new(data: &'a mut [u64]) -> Self {
        Self { data }
    }

    /// Corresponds to `read64`.
    /// Reads 64 bits starting at `bit_offset`.
    pub fn read64(&self, bit_offset: u32) -> u64 {
        let index = (bit_offset >> 6) as usize; // bit_offset / 64
        let shift = bit_offset & 63; // bit_offset % 64

        // These panics are good. C++ would just segfault.
        let lo = self.data[index];
        // Read from data[index + 1], default to 0 if out of bounds
        let hi = self.data.get(index + 1).copied().unwrap_or(0);

        if shift == 0 {
            return lo;
        }

        let safe_shift = (64u32 - shift) & 63u32;

        // Mask to get the bits we need from `hi`
        // C++: (1ull << (shift)) - 1ull
        let test = (1u64.wrapping_shl(shift)).wrapping_sub(1);

        (lo >> shift) | ((hi & test).wrapping_shl(safe_shift))
    }

    /// Corresponds to `write64_masked`.
    /// This is the core write operation.
    pub fn write64_masked(&mut self, bit_offset: u32, value: u64, mask: u64) {
        let index = (bit_offset >> 6) as usize; // bit_offset / 64
        let shift = bit_offset & 63; // bit_offset % 64

        let safe_shift = (64u32 - shift) & 63u32;

        // C++: (mask << shift)
        let mask_write_low = mask.wrapping_shl(shift);
        // C++: ((mask >> safe_shift) & high_mask)
        let mask_write_high = mask.wrapping_shr(safe_shift);

        let value_write_low = value.wrapping_shl(shift);
        let value_write_high = value.wrapping_shr(safe_shift);

        // Apply to data[index] (low part)
        // C++: data[index] = (value << shift) | ((data[index]) & ~mask_write_low);
        self.data[index] = (value_write_low) | (self.data[index] & !mask_write_low);

        // Apply to data[index + 1] (high part)
        if let Some(hi) = self.data.get_mut(index + 1) {
            // C++: data[index + 1] = (((value >> safe_shift)) & mask_write_high) | (data[index + 1] & ~mask_write_high);
            *hi = (value_write_high) | (*hi & !mask_write_high);
        }
    }

    /// Corresponds to `write64_set`
    pub fn write64_set(&mut self, bit_offset: u32, value: u64) {
        self.write64_masked(bit_offset, value, value); // set only bits in value
    }

    /// Corresponds to `write64_unset`
    pub fn write64_unset(&mut self, bit_offset: u32, value: u64) {
        self.write64_masked(bit_offset, 0, value); // clear value bits
    }

    /// Corresponds to `test_function` from 64node.cpp
    pub fn test_function() {
        let mut test_data: Vec<u64> = vec![0; 500];
        {
            let mut bit_stream = Bitstream64::new(&mut test_data);

            let array_offset_0 = 0;
            let array_offset_1 = 65 * 1;
            let array_offset_2 = 65 * 2;
            let array_offset_3 = 65 * 3;
            let array_offset_47 = 65 * 47;
            let array_offset_48 = 65 * 48;

            let leaf_offset = 1; // The 1-bit leaf flag

            let occupancy_test_write =
                0b1011010010110100101101001011010010110100101101001011010010110101u64;
            let occupancy_test_write2 =
                0b1100101110010111001011100101110010111001011100101110010111001011u64;
            let occupancy_test_write3 =
                0b0110101101101010011010110110101001101011011010100110101101101010u64;
            let occupancy_test_write4 = 1u64 << 48;
            let occupancy_test_write5 = 18158512524960202751u64;

            bit_stream.write64_set(array_offset_0 + leaf_offset, occupancy_test_write); // write occupancy
            bit_stream.write64_set(array_offset_0, 1); // write leaf
            let occupancy_mask = bit_stream.read64(array_offset_0 + leaf_offset);

            bit_stream.write64_set(array_offset_1 + leaf_offset, occupancy_test_write2); // write occupancy
            bit_stream.write64_set(array_offset_1, 1); // write leaf
            bit_stream.write64_unset(array_offset_1, 1);
            let occupancy_mask2 = bit_stream.read64(array_offset_1 + leaf_offset);

            bit_stream.write64_set(array_offset_2 + leaf_offset, occupancy_test_write3); // write occupancy
            let occupancy_mask3 = bit_stream.read64(array_offset_2 + leaf_offset);

            bit_stream.write64_set(array_offset_3 + leaf_offset, occupancy_test_write4); // write occupancy
            let occupancy_mask4 = bit_stream.read64(array_offset_3 + leaf_offset);

            bit_stream.write64_set(array_offset_47 + leaf_offset, occupancy_test_write4); // write occupancy
            let occupancy_mask5 = bit_stream.read64(array_offset_47 + leaf_offset);

            bit_stream.write64_set(array_offset_48 + leaf_offset, occupancy_test_write5); // write occupancy
            bit_stream.write64_set(array_offset_48, 1);
            let occupancy_mask6 = bit_stream.read64(array_offset_48 + leaf_offset);

            assert_eq!(occupancy_mask, occupancy_test_write);
            assert_eq!(occupancy_mask2, occupancy_test_write2);
            assert_eq!(occupancy_mask3, occupancy_test_write3);
            assert_eq!(occupancy_mask4, occupancy_test_write4);
            assert_eq!(occupancy_mask5, occupancy_test_write4);
            assert_eq!(occupancy_mask6, occupancy_test_write5);

            assert_eq!(bit_stream.read64(array_offset_0) & 1, 1);
            assert_eq!(bit_stream.read64(array_offset_1) & 1, 0);
            assert_eq!(bit_stream.read64(array_offset_2) & 1, 0);
            assert_eq!(bit_stream.read64(array_offset_48) & 1, 1);
        }
        log::info!("Bitstream64 test passed!");
    }
}

/// Corresponds to `nodeutil` namespace
pub mod node_util {
    use super::Bitstream64;
    use glam::IVec3;

    /// Each node (leaf flag + occupancy mask) occupies 65 bits.
    pub const NODE_TOTAL_BITS: u32 = 65;

    /// Offset of the 64-bit occupancy mask within the 65-bit node.
    pub const OCCUPANCY_OFFSET: u32 = 1;

    /// Bitmask for the leaf flag (at bit 0).
    pub const LEAF_MASK: u64 = 1;

    /// Corresponds to `get_local_index`
    #[inline(always)]
    pub fn get_local_index(x: u32, y: u32, z: u32) -> u32 {
        // 4x4x4 grid = 64 children
        let child_bit = x + (y << 2) + (z << 4);
        assert!(child_bit < 64);
        child_bit
    }

    /// Corresponds to `get_child_pos_from_local`
    #[inline(always)]
    pub fn get_child_pos_from_local(child_bit: u32) -> IVec3 {
        assert!(child_bit < 64);
        let x = child_bit & 0b11;
        let y = (child_bit >> 2) & 0b11;
        let z = (child_bit >> 4) & 0b11;
        IVec3::new(x as i32, y as i32, z as i32)
    }

    /// Corresponds to `is_leaf`
    #[inline(always)]
    pub fn is_leaf(nodes: &mut [u64], array_index: u32) -> bool {
        let tot_bit_offset = array_index * NODE_TOTAL_BITS;
        // We only need to read 1 bit, so `read64` is overkill but correct.
        let mut stream = Bitstream64::new(nodes);
        (stream.read64(tot_bit_offset) & LEAF_MASK) != 0
    }

    /// Corresponds to `get_occupancy_mask`
    #[inline(always)]
    pub fn get_occupancy_mask(nodes: &mut [u64], array_index: u32) -> u64 {
        let parent_bit_index = array_index * NODE_TOTAL_BITS + OCCUPANCY_OFFSET;
        let mut stream = Bitstream64::new(nodes);
        stream.read64(parent_bit_index)
    }

    /// Corresponds to `set_leaf`
    #[inline(always)]
    pub fn set_leaf(nodes: &mut [u64], array_index: u32) {
        let array_total_bits = array_index * NODE_TOTAL_BITS;
        let mut stream = Bitstream64::new(nodes);
        stream.write64_set(array_total_bits, LEAF_MASK);
    }

    /// Corresponds to `unset_leaf`
    #[inline(always)]
    pub fn unset_leaf(nodes: &mut [u64], array_index: u32) {
        let array_total_bits = array_index * NODE_TOTAL_BITS;
        let mut stream = Bitstream64::new(nodes);
        stream.write64_unset(array_total_bits, LEAF_MASK);
    }

    /// Corresponds to `set_occupancy_bit`
    #[inline(always)]
    pub fn set_occupancy_bit(nodes: &mut [u64], array_index: u32, local: IVec3) {
        let relative_offset = get_local_index(local.x as u32, local.y as u32, local.z as u32);
        let array_total_bits = array_index * NODE_TOTAL_BITS;
        let mut stream = Bitstream64::new(nodes);
        stream.write64_set(array_total_bits + OCCUPANCY_OFFSET, 1u64 << relative_offset);
    }

    /// Corresponds to `set_occupancy_mask`
    #[inline(always)]
    pub fn set_occupancy_mask(nodes: &mut [u64], array_index: u32, occupancy_mask: u64) {
        let array_total_bits = array_index * NODE_TOTAL_BITS;
        let mut stream = Bitstream64::new(nodes);
        stream.write64_set(array_total_bits + OCCUPANCY_OFFSET, occupancy_mask);
    }

    /// Corresponds to `clear_occupancy_mask`
    #[inline(always)]
    pub fn clear_occupancy_mask(nodes: &mut [u64], array_index: u32) {
        let array_total_bits = array_index * NODE_TOTAL_BITS + OCCUPANCY_OFFSET;
        let mut stream = Bitstream64::new(nodes);
        stream.write64_unset(array_total_bits, u64::MAX);
    }
}
