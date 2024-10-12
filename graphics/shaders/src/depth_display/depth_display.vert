#version 460 core

#define VULKAN 100

layout (location = 0) in vec3 pos;
layout (location = 1) in vec3 color;
layout (location = 2) in vec3 norm;
layout (location = 3) in vec2 uv;

layout (location = 0) out VS_OUT {
    vec2 uv;
} vs_out;

void main()
{
    vs_out.uv = uv;
    gl_Position = vec4(pos, 1.0);
}
