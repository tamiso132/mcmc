// A simple compute shader in Slang

// Define our input and output buffers.
// We'll read from A and B, and write to Out.
StructuredBuffer<float> g_a : register(t0);
StructuredBuffer<float> g_b : register(t1);
RWStructuredBuffer<float> g_out : register(u0);

// The main entry point for the compute shader.
// We'll launch this in 1D, with [numThreads(1, 1, 1)].
[shader("compute")]
[numThreads(64, 1, 1)]
void computeMain(uint3 dispatchThreadID : SV_DispatchThreadID)
{
    // Get the index for the current thread
    uint index = dispatchThreadID.x;
    
    // Read from the input buffers
    float a = g_a[index];
    float b = g_b[index];
    
    // Add them and write to the output buffer
    g_out[index] = a + b;
}