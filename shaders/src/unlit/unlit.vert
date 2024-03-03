// #extension GL_KHR_vulkan_glsl : require
#version 460 core
#define VULKAN 100

layout(location = 0) in vec3 pos;
layout(location = 1) in vec3 color;

layout(location = 0) out vec3 vert_color;

layout(push_constant) uniform transform {
  mat4 proj;
  mat4 view;
  mat4 model;
}
m;

void main() {
  gl_Position = m.proj * m.view * m.model * vec4(pos, 1.0);
  vert_color = color;
}
