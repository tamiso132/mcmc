#version 450
layout(row_major) uniform;
layout(row_major) buffer;

#line 5 0
struct Test_0
{
    int g_0;
    int h_0;
};


layout(std430, binding = 1) readonly buffer StructuredBuffer_Test_t_0 {
    Test_0 _data[];
} g_a_0[];
layout(std430, binding = 1) readonly buffer StructuredBuffer_float_t_0 {
    float _data[];
} g_b_0[];
layout(std430, binding = 0) buffer StructuredBuffer_float_t_1 {
    float _data[];
} g_out_0[];



layout(local_size_x = 1, local_size_y = 1, local_size_z = 1) in;
void main()
{

#line 34
    g_out_0[3]._data[uint(0)] = float(g_a_0[1]._data[uint(0)].g_0) + g_b_0[2]._data[uint(0)];
    return;
}

