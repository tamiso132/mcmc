
#pragma once

#include "shared/rayshared.inl"

#include "rayutil/camera.slang"
#include "rayutil/raytrace.slang"
#include "rayutil/shared.slang"

static const float4 SKY_COLOR = float4(0.529, 0.808, 0.922, 1.0);


struct SamplingData {
    float3 sample_origin;
    float3 normal;
    bool is_hit;
};

groupshared SamplingData shared_data[64];


struct ComputeInput {
    uint3 global_id: SV_DispatchThreadID;
    uint3 local_id: SV_GroupID;
}

//[[vk::push_constant]]
//RayPush pc;

//DAXA_DECL_STORAGE_BUFFERS StructuredBuffer<RayCam> ray_cam;

[shader("compute")]
[numthreads(8, 8, 1)] // workgroup size
void compute_main(ComputeInput input)
{
    let coord = input.global_id.xy;
    if(coord.x > pc.extent.x || coord.y > pc.extent.y){
        return;
    }
    let cam = ImplRayCam(*pc.cam_adress);

   
    
    let ray = cam.get_ray(pc.extent, coord);
    RayTraceConstState const_state = RayTraceConstState(ray);

    RWTexture2D<float4>::get(pc.img_output_id).Store(coord, float4(ray.dir, 1.0));
    
    do{
    
        var raytrace_state = ErrTry RaytraceState::create(pc.nodes_id, pc.entry_id, pc.capacity, const_state, float3(0,0,0), ray, float2(coord) / float2(pc.extent));
        let hit_record = ErrTry raytrace_state.trace_ray(const_state, 1000000.0);
        
        
        if(!hit_record.is_hit){
            RWTexture2D<float4>::get(pc.img_output_id).Store(coord, float4(ray.dir, 1.0));
        }
        else{
             RWTexture2D<float4>::get(pc.img_output_id).Store(coord, SKY_COLOR);
        }
    }

    catch(err: Error::Raytrace){
        switch(err){
            case Error::Raytrace::HashMapFailure:
           //  RWTexture2D<float4>::get(pc.img_output_id).Store(coord, float4(ray.dir, 1.0));
            break;
        };
       // RWTexture2D<float4>::get(pc.img_output_id).Store(coord, float4(ray.dir, 1.0));
    }
    
    
    
    
}