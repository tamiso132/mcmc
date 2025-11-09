
const static float2 UVs[6] = float2[6]( //
    float2(1.0, 0.0),                   //
    float2(1.0, 1.0),                   //
    float2(0.0, 0.0),                   //
    float2(1.0, 1.0),                   //
    float2(0.0, 1.0),                   //
    float2(0.0, 0.0), );

const static float2 Positions[6] = {
    float2(1.0, 1.0),
    float2(1.0, -1.0),
    float2(-1.0, 1.0),
    float2(1.0, -1.0),
    float2(-1.0, -1.0),
    float2(-1.0, 1.0)
};

struct VertexOutput
{
    float2 uv : UV;
    float4 sv_position : SV_Position;
};

[vk_binding(0, 0)]
Sampler2D tex_0;

[shader("vertex")]
VertexOutput vs_main(uint vertex_index: SV_VulkanVertexID)
{
    VertexOutput output;
    output.uv = UVs[vertex_index];
    output.sv_position = float4(Positions[vertex_index], 0.0, 1.0);

    return output;
}

[shader("fragment")]
float4 fs_main(float2 uv: UV)
{
    return float4(tex_0.Sample(uv).xyz, 1.0);
}

