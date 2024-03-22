// #extension GL_KHR_vulkan_glsl : require
#version 460 core
#define VULKAN 100

layout(location = 0) in vec3 pos;
layout(location = 1) in vec3 color;
layout(location = 2) in vec3 norm;
layout(location = 3) in vec2 uv;

layout(location = 0) out struct VS_OUT {
  vec3 color;
  vec2 uv;
} vs_out;

layout(push_constant) uniform transform {
  mat4 proj;
  mat4 view;
  mat4 model;
}
m;

void main() {
  gl_Position = m.proj * m.view * m.model * vec4(pos, 1.0);
  vs_out.color = color;
  vs_out.uv = uv;
}
