#pragma once

static const float PIOver4 = 0.78539816339744830961;
static const float PiOver2 = 1.57079632679489661923;


namespace Random{
    float2 hash2(uint seed){
        let s = seed * 12.9898 + 78.233;
        return fract(sin(float2(s, s+ 1.0)) * float2(43758.5453123, 22578.1459123));
    }

    float2 wey12(uint i){
        let alpha = 0.61803398875; // golden ratio conjugate
        let beta = 0.7548776662467; // another irrational for y
        return fract(float2(float(i) * alpha, float(i) * beta)); 
    }

    float2 remap(float2 x, float max) {
        return (x * 2.0 - 1.0) * max;
    }
}

namespace Sample{

    public float3 generate_direction(uint pixel_index, uint sample_index, float3 normal){
        let uv = Random::wey12(sample_index);
        let uv_concentric = concentric_sample_disk(uv);
        let dir = cosine_hemisphere(uv_concentric);

        //TODO need to fix this for flipped
        let up = select(abs(normal.z) > 0.999, float3(1.0, 0.0, 0.0), float3(0.0, 0.0, 0.1));
        let tangent = normalize(cross(up, normal));
        let bit_tangent = cross(normal, tangent);
        return dir.x * tangent + dir.y * bit_tangent + dir.z * normal;


    }

    float3 cosine_hemisphere(float2 uv){
        let z = sqrt(max(0.0, 1.0 - uv.x * uv.x - uv.y * uv.y));
        return normalize((float3(uv.x, uv.y, z)));
    }

    float2 concentric_sample_disk(float2 uv){
        var u_offset = Random::remap(uv, 1.0);

        var r = 0.0;
        var theta = 0.0;

        if (u_offset.x == 0.0 && u_offset.y == 0.0) {
            return u_offset;
        }

        if (abs(u_offset.x) > abs(u_offset.y)) {
            r = u_offset.x;
            theta = PIOver4 * (u_offset.y / u_offset.x);
        } else {
            r = u_offset.y;
            theta = PiOver2 - PIOver4 * (u_offset.x / u_offset.y);
        }

        return r * float2(cos(theta), sin(theta));
    }
}