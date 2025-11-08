#pragma once

#include "shared/rayshared.inl"
#include "shared.slang"


public struct ImplRayCam {

    __init(RayCam cam){
        this.cam.w = cam.w;
        this.cam.v = cam.v;
        this.cam.u = cam.u;
        this.cam.pos = cam.pos;
    }
    public Ray get_ray(int2 extent, uint2 coord){
        let fov = radians(90.0); // increase for wider FOV
        let focal_length = 1.0 / tan(fov * 0.5);

        let aspect_ratio = float(extent[0]) / float(extent[1]);

        let uv = ((float2(coord) + 0.5) / float2(extent)) * 2.0 - float2(1.0, 1.0);
        let ndc = float2(uv.x * aspect_ratio, -uv.y) / focal_length;

        let uv_sample = float2(coord) / float2(extent);



        let rotation_test = matrix<float, 3, 3, 3>(this.cam.u.xyz, this.cam.v.xyz, this.cam.w.xyz);
        var ray = Ray();
        ray.origin = this.cam.pos.xyz;
        ray.dir = mul(float3(ndc, 1.0), rotation_test);

        return ray;
    }

    RayCam cam;
};