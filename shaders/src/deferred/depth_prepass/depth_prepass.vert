// #extension GL_KHR_vulkan_glsl : require
// #extension GL_KHR_vulkan_glsl : require
#version 460 core
#define VULKAN 100

layout(location = 0) in vec3 pos;
layout(location = 1) in vec3 color;
layout(location = 2) in vec3 norm;
layout(location = 3) in vec2 uv;
layout(location = 4) in vec4 tangent;

layout(push_constant) uniform transform { mat4 model; }
m;

layout(set = 0, binding = 0) uniform camera {
  mat4 view;
  mat4 proj;
}
c;

void main() {
  gl_Position = c.proj * c.view * m.model * vec4(pos, 1.0);
}
