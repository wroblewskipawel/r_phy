#version 460 core

#define VULKAN 100

layout(location = 0) in vec3 pos;
layout(location = 1) in vec3 norm;
layout(location = 2) in vec3 color;
layout(location = 3) in vec2 uv;

void main() {
    gl_Position = vec4(pos, 1.0);
}
