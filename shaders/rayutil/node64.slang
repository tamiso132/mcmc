#pragma once

#include "shared/rayshared.slang"
#include "rayutil/shared.slang"

static const uint NODE_TOT_BITS = 65;

public uint get_local_index(int3 local_pos)
{
    return local_pos.x + (local_pos.y << 2u) + (local_pos.z << 4u);
}

public float3 get_local_pos(uint local_index)
{
    let x = local_index & 0x3u;
    let y = (local_index >> 2u) & 0x3u;
    let z = (local_index >> 4u) & 0x3u;

    return int3(x, y, z);
}

namespace Bit
{

uint trailing_zero(uint64_t val)
{
    var mask_u32 = (1ull << 32u) - 1ull;
    let low = firstbithigh(uint(val & mask_u32));
    let high = firstbithigh(uint((val >> 32u) & mask_u32)) + 32u;
    let has_low = low != 32u;
    return select(has_low == true, low, high);
}

uint count_bits_lsb(uint64_t value, uint32_t bit_limit)
{
    let mask_u32 = uint(((1ull << 32) - 1ull));
    let value_low = uint(value & uint64_t(mask_u32));
    let value_high = uint((value >> 32) & uint64_t(mask_u32));

    let is_low = bit_limit < 32u;

    let shift_low = bit_limit;
    let shift_high = select(is_low, 0u, bit_limit - 32u);

    let mask_low = select(is_low, uint((1ull << shift_low) - 1ull), mask_u32);

    let mask_high = select(is_low, 0u, uint((1ull << shift_high) - 1ull), );

    let masked_low = value_low & mask_low;
    let masked_high = value_high & mask_high;

    return countbits(masked_low) + countbits(masked_high);
}
}

public struct ImplNodeKey
{

    static bool is_equal(NodeKey self, NodeKey other)
    {
        return other.min_corner.x == self.min_corner.x && other.min_corner.y == self.min_corner.y && other.min_corner.z == self.min_corner.z && self.level == other.level;
    }

    static bool is_empty(NodeKey self)
    {
        return any(float3(-1.0, -1.0, -1.0) == self.min_corner) && self.level == 0xFFFFFFFEu;
    }

    static bool is_removed_key(NodeKey self)
    {
        return any(float3(-1.0, -1.0, -1.0) == self.min_corner) && self.level == 0xFFFFFFFDu;
    }
};

struct Hash
{

    static uint fnv1a(NodeKey key)
    {
        var h = 2166136261u;
        h ^= scramble(bit_cast<uint>(key.min_corner.x));
        h *= 16777619u;
        h ^= scramble(bit_cast<uint>(key.min_corner.y));
        h *= 16777619u;
        h ^= scramble(bit_cast<uint>(key.min_corner.z));
        h *= 16777619u;
        h ^= key.level;
        h *= 16777619u;
        return h;
    }

    static uint scramble(uint x)
    {
        var v = x;
        v ^= v >> 16u;
        v *= 2246822507u;
        v ^= v >> 13u;
        v *= 3266489909u;
        v ^= v >> 16u;
        return v;
    }
}

struct GPUHashmap
{
    static uint get(RWStructuredBuffer<HashEntry> entries, NodeKey key, uint capacity) throws Error::Raytrace
    {
        uint mask = capacity - 1u;
        var index = Hash::fnv1a(key) & mask;

        for (var i = 0; i < capacity; i++)
        {
            var probe = (i + index) & mask;
            if (ImplNodeKey::is_empty(entries[i].key))
            {
                return uint::maxValue;
            }
            else if (ImplNodeKey::is_equal(entries[i].key, key))
            {
                return entries[i].value;
            }
        }

        ErrReturn Error::err_ret(Error.Raytrace.HashMapFailure, uint.maxValue);
    }
};

struct Node64
{

    static uint64_t get_occupancy_mask(RWStructuredBuffer<uint64_t> nodes, uint array_index)
    {
        let tot_bit_offset = array_index * NODE_TOT_BITS + 1u;
        return unsafe_read64(nodes, tot_bit_offset);
    }

    static bool is_leaf(RWStructuredBuffer<uint64_t> nodes, uint array_index)
    {
        let total_bits = NODE_TOT_BITS * 65;

        let index = total_bits >> 6u;
        let index_offset = total_bits & 63;

        return bool((nodes[index] >> index_offset) & 1);
    }

    static void first_child_array_index(RWStructuredBuffer<HashEntry> entries, uint capacity, uint64_t p_mask, float3 p_min_corner, int p_level, float child_scale, inout uint array_index) throws Error::Raytrace
    {

        if (p_mask != 0ull)
        {

            let local_index = Bit::trailing_zero(p_mask);
            var local_pos = get_local_pos(local_index);

            var key = NodeKey(child_scale * local_pos + p_min_corner, p_level - 1);
            array_index = ErrTry get_array_index(entries, key, capacity);
        }
        else
        {
            // TODO, make function throw an error
            array_index = 0u;
        }
    }

    static uint get_array_index_by_relative(uint64_t mask, int3 local_pos, uint relative_index)
    {
        let local_index = get_local_index(local_pos);
        let local_array_index = select(mask != 0, Bit::count_bits_lsb(mask, local_index), 0u);
        return local_array_index + relative_index;
    }

    static uint get_array_index(RWStructuredBuffer<HashEntry> entries, NodeKey key, uint capacity) throws Error::Raytrace
    {
        return ErrTry GPUHashmap::get(entries, key, capacity);
    }

    private static uint64_t unsafe_read64(RWStructuredBuffer<uint64_t> nodes, uint bit_offset)
    {
        var index = bit_offset >> 6u;
        var shift = bit_offset & 63u;

        var safe_shift = (64u - shift) & 63u;
        var high_mask = (1ull << shift) - 1ull;

        // if no shift, then hi_index is dont care
        high_mask = select(shift == 0u, 0ull, high_mask);
        var hi_index = select(shift == 0u, index + 1u, index);

        let lo = nodes[index];
        let hi = nodes[hi_index];

        return (lo >> shift) | ((hi & high_mask) << safe_shift);
    }
};
