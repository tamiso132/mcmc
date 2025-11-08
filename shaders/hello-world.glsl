#version 450
layout(row_major) uniform;
layout(row_major) buffer;

#line 29 0
layout(binding = 0)
uniform sampler2D tex_0_0;


#line 1015 1
layout(location = 0)
out vec4 entryPointParam_fs_main_0;


#line 1015
layout(location = 0)
in vec2 uv_0;


#line 42 0
void main()
{

#line 42
    entryPointParam_fs_main_0 = vec4((texture((tex_0_0), (uv_0))).xyz, 1.0);

#line 42
    return;
}

