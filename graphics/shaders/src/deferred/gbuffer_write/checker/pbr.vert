#version 460 core

#define VULKAN 100

layout(location = 0) in vec3 pos;
layout(location = 1) in vec3 color;
layout(location = 2) in vec3 norm;
layout(location = 3) in vec2 uv;
layout(location = 4) in vec4 tangent;

layout(location = 0) out VS_OUT {
    vec3 pos;
    vec3 norm;
    vec3 color;
    vec2 uv;
} vs_out;

layout(set = 0, binding = 0) uniform Camera {
    mat4 view;
    mat4 proj;
} c;

layout(push_constant) uniform Model {
    mat4 model;
    mat3 model_inv_t;
} m;

void main() {
    vec4 world_pos = m.model * vec4(pos, 1.0);
    vec3 world_norm = m.model_inv_t * norm;
    vs_out.pos = world_pos.xyz;
    vs_out.norm = world_norm;
    vs_out.color = color;
    vs_out.uv = uv;
    gl_Position = c.proj * c.view * world_pos;
}
