// A simple compute shader in Slang

// Define our input and output buffers.
// We'll read from A and B, and write to Out
struct Test
{
    int g;
    int h;
}

[vk_binding(1, 0)]
StructuredBuffer<Test> g_a[];

[vk_binding(1, 0)]
StructuredBuffer<float> g_b[];

[vk_binding(0, 0)]
RWStructuredBuffer<float> g_out[];



// The main entry point for the compute shader.
// We'll launch this in 1D, with [numThreads(1, 1, 1)].
[shader("compute")]
[numthreads(1, 1, 1)]
void computeMain(uint3 dispatchThreadID: SV_DispatchThreadID)
{
    // Get the index for the current thread
    uint index = dispatchThreadID.x;

    // Read from the input buffers
    Test a = g_a[1][0];
    float b = g_b[2][0];

    // Add them and write to the output buffer
    g_out[3][0] = a.g + b;
}
