#version 460 core

#define VULKAN 100

layout(location = 0) in vec3 pos;
layout(location = 1) in vec3 color;
layout(location = 2) in vec3 norma;
layout(location = 3) in vec2 uv;

layout(location = 0) out VS_OUT {
    vec3 color;
    vec3 norma;
    vec2 uv;
} vs_out;

layout(set = 0, binding = 0) uniform Camera {
    mat4 view;
    mat4 proj;
} c;

layout(push_constant) uniform Model {
    mat4 model;
} m;

void main() {
    gl_Position = c.proj * c.view * m.model * vec4(pos, 1.0);
    vs_out.color = color;
    vs_out.norma = norma;
    vs_out.uv = uv;
}
