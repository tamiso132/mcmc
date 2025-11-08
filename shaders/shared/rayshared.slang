#include "shader/shadertypes.hpp"
#include <shader/shader.inl>

typedef float atomic_slang;

#define SHADER_MAX_TREE_LEVEL 4


struct SHADER_ALIGN_16 RayCam {
  float4 scaled_img_size;
  float4 pos;
  float4 w;
  float4 v;
  float4 u;
  float4 d_delta_u;
  float4 d_delta_v;
};

struct NodeKey {
  float3 min_corner;
  uint level;
};

struct SHADER_ALIGN_16 HashEntry {
  NodeKey key;
  uint value;
};

struct SHADER_ALIGN_16 SamplingResult {

  float sum_normal_x;
  float sum_normal_y;
  float sum_normal_z;
  float normal_hit_count;
  float ambient_hit_count;
  float normal_non_hit_count;
  float invalid_hit_count;
};

struct SHADER_ALIGN_16 RayPush {
  // vec3 -> 12 bytes, but aligned to 16
  float3 root_min; // offset 0
  uint max_level;  // offset 12 → fits into padding, now 16 total

  int2 extent;    // offset 16
  int root_size;  // offset 24
  int capacity;   // offset 28
  int debug_mode; // offset 32
  uint frame;     // offset 36
  int2 padding;

  // 64-bit device addresses (8 bytes each, padded to 16)
  uint64_t cam_id;      // offset 48 (8 used, 8 padding)
  uint64_t sampling_id; // offset 64
  uint64_t nodes_id;    // offset 80
  uint64_t entry_id;    // offset 96

  // image handles are usually 64-bit IDs (same rules)
  uint64_t img_output_id; // offset 112
};

// #define SHADER_MAX_TREE_LEVEL 4

// #if defined(__cplusplus)
//   }
// }
// #endif
} // namespace shader