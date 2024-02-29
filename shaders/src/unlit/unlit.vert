#version 460 core
#define VULKAN 100

layout(location = 0) in vec3 pos;
layout(location = 1) in vec3 color;

layout(location = 0) out vec3 vert_color;

void main() {
  gl_Position = vec4(pos, 1.0);
  vert_color = color;
}
