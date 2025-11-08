
#pragma once

#include "shared/rayshared.inl"

#include "shared.slang"
#include "node64.slang"

static const float VOXEL_SCALE = 1.0;
static const uint MAX_STACK_LEN = 5;


int homebrew_dot(int3 t, int3 other){
   let r =  t * other;
   return r.x + r.y + r.z;
}

struct NodeStack{
    float3 min_corner;
    float t_tot;
    int3 local_pos;
    uint child_array_index;
    float3 side_dist;
    uint64_t mask;
};


public struct HitRecord{
    public float3 normal;
    public float t_tot;
    public uint steps;
    public bool is_hit;
    public bool is_invalid;
}

struct BranchlessDDA {
    float3 step_dir;
    float voxel_size;
    float3 side_dist;
    float t_relative;
    float3 delta_dist;
    float3 local_pos;
    bool out_of_bounds;
    float3 prev_step;

    [mutating]
    void init(float3 step, float voxel_size) {
        step_dir = step;
        voxel_size = voxel_size;
        out_of_bounds = false;
    }
    
};



public struct RayTraceConstState{

    public __init(Ray ray){
        
        this.S = step(float3(0.0, 0.0, 0.0), ray.dir);
        this.step_dir = 2.0 * S - 1.0;
        this.step_dir_i32 = int3(step_dir);
        this.max_level = SHADER_MAX_TREE_LEVEL;

        for (var i = 0; i < 3; i++) {
            if(ray.dir[i] >= 0.0) {
                this.step_dir_flag[i] = 1.0;
                this.step_dir_neg_flag[i] = 0.0;
            } else {
                this.step_dir_flag[i] = 0.0;
                this.step_dir_neg_flag[i] = 1.0;
            }
        }
    }

    float3 step_dir;
    int3 step_dir_i32;
    float3 step_dir_flag;
    uint max_level;
    float3 step_dir_neg_flag;
    float3 S;
}

struct RaytraceState{
   
    static RaytraceState create(uint64_t *nodes, HashEntry *entries, uint capacity, RayTraceConstState const_state, float3 center, Ray ray, float2 uv) ErrThrow
     {
        var this = RaytraceState();
        this.child_scale = VOXEL_SCALE * (1u << (const_state.max_level*2));
        this.parent_scale = this.child_scale * 4.0;
       
        this.nodes = nodes;
        this.entries = entries;
        this.capacity = capacity;
        this.prev_step = float3(1.0, 0.0, 0.0);
        this.is_occupied = false;
       
        for(uint i = 0; i < MAX_STACK_LEN; i++){
            this.parent_stack[i].child_array_index = 0;
            this.parent_stack[i].local_pos = int3(0, 0, 0);
            this.parent_stack[i].mask = 0;
            this.parent_stack[i].min_corner = float3(0, 0, 0);
            this.parent_stack[i].t_tot = 0;
            this.parent_stack[i].side_dist = float3(0,0,0);
        }

        float3 S = step(float3(0.0, 0.0, 0.0), ray.dir);

        this.t_tot = 0.0;
        this.level = 0;
        this.min_corner = center - (this.parent_scale / 2.0);
        this.parent_index = 0u;

        this.delta_dist = select(ray.dir == float3(0.0), float3(1e30), this.child_scale / abs(ray.dir));
        float3 local_pos_frac = (ray.origin - this.min_corner) / this.child_scale;
        this.side_dist = (S - const_state.step_dir * fract(local_pos_frac)) * this.delta_dist;
        
        this.local_pos = int3(floor(local_pos_frac));
        this.out_of_bounds = any(this.local_pos > int3(3, 3, 3)) || any(this.local_pos < int3(0, 0, 0));
        
        bool3 is_next_voxel = bool3(this.side_dist[0] == 0.0, this.side_dist[1] == 0.0, this.side_dist[2] == 0.0);
        this.local_pos = select(is_next_voxel, this.local_pos + const_state.step_dir_i32, this.local_pos);
        this.side_dist = select(is_next_voxel,this.delta_dist, this.side_dist);
        
        NodeKey key;
        key.min_corner = this.min_corner;
        key.level = this.level;
        let array_index = ErrTry Node64::get_array_index(entries, key, capacity);
        // this.occupancy_mask = Node64::get_occupancy_mask(nodes, array_index);
        // ErrTry Node64::first_child_array_index(entries, capacity, this.occupancy_mask, this.min_corner, this.level, this.child_scale, this.child_array_index);
        
        return this;
    }

    [mutating]
    HitRecord trace_ray(RayTraceConstState const_state, float t_max) throws Error::Raytrace{
        

        while(true){
            while(!this.out_of_bounds)
            {
                if(const_state.max_level <= this.level){
                    HitRecord hit_record;
                    hit_record.is_hit = false;
                    hit_record.t_tot = this.t_tot;
                    hit_record.is_invalid = false;
                    hit_record.steps = 0;

                    return hit_record;
                }

                ErrTry this.ascend(const_state);
            }

            while(this.out_of_bounds && !this.is_occupied){ // step
                step(const_state);

                if (this.t_tot > t_max && !this.is_occupied) {
                    HitRecord hit_record;
                    hit_record.is_hit = false;
                    hit_record.is_invalid = false;

                    return hit_record;
                }
            }

           

            while(this.level  > 0 && !this.out_of_bounds && this.is_occupied){
                HitRecord hit_record = descend_is_leaf();

                if(hit_record.is_hit){
                    // TODO, normal calculation
                    return hit_record;
                }

                ErrTry this.descend(const_state);
            }

            // HitRecord test;
            // test.is_hit = true;
            // return test;
        }
        
    }

    [mutating]
    private void step(RayTraceConstState const_state){
        let conds = step(this.side_dist.xxyy, this.side_dist.yzzx);

        var cases = float3(0.0, 0.0, 0.0);
        cases.x = conds.x * conds.y;
        cases.y = (1.0 - cases.x) * conds.z * conds.w;
        cases.z = (1.0 - cases.x) * (1.0 - cases.y);

        let cases_i32 = int3(cases);
        
        this.local_pos += const_state.step_dir_i32 * cases_i32;
        this.prev_step = -const_state.step_dir_i32 * cases_i32;

        let local_value = homebrew_dot(this.local_pos, cases_i32);
        this.out_of_bounds = local_value > 3 || local_value < 0;

        var t_traveled = dot(this.side_dist, cases);
        this.t_tot += t_traveled;
        this.side_dist -= t_traveled;
        this.side_dist += cases * this.delta_dist;
        
        let local_index_check = get_local_index(this.local_pos);
        this.is_occupied = bool((this.occupancy_mask >> local_index_check) & 1);

    }

    [mutating]
    private void ascend(RayTraceConstState const_state) throws Error::Raytrace{
        
        this.parent_index -= 1u;
        this.level += 1u;

        this.min_corner = parent_stack[this.parent_index].min_corner;
        this.side_dist = parent_stack[this.parent_index].side_dist;
        this.local_pos = parent_stack[this.parent_index].local_pos;

        this.occupancy_mask = parent_stack[this.parent_index].mask;
        let t_during_descend = parent_stack[this.parent_index].t_tot;

        let parent_delta = this.delta_dist * 4.0;

        this.side_dist = this.side_dist - this.t_tot + t_during_descend;
        let val_smallest_axis = min(min(this.side_dist[0], this.side_dist[1]), this.side_dist[2]);

        let epsilon = 1e-3;

        let is_next_voxel = bool3(this.side_dist[0] <= epsilon, this.side_dist[1] <= epsilon, this.side_dist[2] <= epsilon);
        let is_next = is_next_voxel[0] || is_next_voxel[1] || is_next_voxel[2];
        this.local_pos = select(is_next_voxel, this.local_pos + const_state.step_dir_i32, this.local_pos);
        this.t_tot = select(is_next, this.t_tot + val_smallest_axis, this.t_tot);
        this.side_dist = select(is_next, this.side_dist - val_smallest_axis, this.side_dist);
        this.side_dist = select(is_next_voxel, parent_delta, this.side_dist);

        this.delta_dist = parent_delta;
        this.child_scale = this.parent_scale;
        this.parent_scale = this.parent_scale * 4.0;

        this.out_of_bounds = any(this.local_pos > int3(3, 3, 3)) || any(this.local_pos < int3(0,0,0));
    }

   

    [mutating]
    private void descend(RayTraceConstState const_state) throws Error::Raytrace{
        this.parent_stack[this.parent_index].min_corner = this.min_corner;
        this.parent_stack[this.parent_index].local_pos = this.local_pos;
        this.parent_stack[this.parent_index].side_dist = this.side_dist;
        this.parent_stack[this.parent_index].t_tot = this.t_tot;
        this.parent_stack[this.parent_index].child_array_index = this.child_array_index;
        this.parent_stack[this.parent_index].mask = this.occupancy_mask;
        this.parent_index += 1u;

        this.min_corner = this.min_corner + float3(this.local_pos) * this.child_scale;
        this.level -= 1;
        let child_array_index = Node64::get_array_index_by_relative(this.occupancy_mask, this.local_pos, this.child_array_index);
        
        this.occupancy_mask = Node64::get_occupancy_mask(nodes, child_array_index);

        let child_delta = this.delta_dist / 4.0;
        this.parent_scale = this.child_scale;
        this.child_scale /= 4.0;
        
        // this might not be correct
        let dist_traveled = clamp((this.delta_dist - this.side_dist) / this.child_scale, float3(0.0, 0.0, 0.0), float3(3.999, 3.999, 3.999));
        let local_pos_frac = abs(4.0 * const_state.step_dir_neg_flag - dist_traveled);

        // if dir positive, goes toward 1 otherwise toward 0
        this.side_dist = select(const_state.step_dir_i32 == int3(1, 1, 1), fract(local_pos_frac), 1.0 - fract(local_pos_frac));
        this.local_pos = int3(floor(local_pos_frac));
        let epsilon = 1e-6;
        let is_next_voxel = bool3(this.side_dist[0] <= epsilon, this.side_dist[1] <= epsilon, this.side_dist[2] <= epsilon);
        let is_next = any(is_next_voxel);
        this.local_pos = select(is_next_voxel, this.local_pos + const_state.step_dir_i32, this.local_pos);
        // TODO, might need to also subtract from sideDist
        this.t_tot = select(is_next, t_tot + epsilon, t_tot);
        this.side_dist = select(is_next_voxel, child_delta, this.side_dist);

        ErrTry Node64::first_child_array_index(entries, capacity, this.occupancy_mask, this.min_corner, this.level, this.child_scale, this.child_array_index);
        this.delta_dist = child_delta;
        
        let local_index_check = get_local_index(this.local_pos);
        is_occupied = bool((this.occupancy_mask >> local_index_check) & 1);
    }

    private HitRecord descend_is_leaf(){
        let child_array_index = Node64::get_array_index_by_relative(this.occupancy_mask, this.local_pos, this.child_array_index);

        HitRecord hit_record;
        hit_record.is_hit = false;
        hit_record.is_invalid = false;
        hit_record.is_hit = Node64::is_leaf(nodes, child_array_index);


        return hit_record;
    }

    float3 min_corner;
    float child_scale;
    float3 side_dist;
    float t_tot;
    float3 delta_dist;
    float parent_scale;
    int3 local_pos;
    int level;
    float3 prev_step;
    uint parent_index;
    uint64_t occupancy_mask;
    uint child_array_index;
    bool out_of_bounds;
    NodeStack parent_stack[MAX_STACK_LEN];

    bool is_occupied;

    //constants ish
    uint capacity;
    HashEntry* entries;
    uint64_t* nodes;

};