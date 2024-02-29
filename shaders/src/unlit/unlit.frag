#version 460 core
#define VULKAN 100

layout(location = 0) in vec3 vert_color;

layout(location = 0) out vec4 frag_color;

void main() { frag_color = vec4(vert_color, 1.0); }
