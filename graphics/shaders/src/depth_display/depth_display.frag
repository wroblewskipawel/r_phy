#version 460 core

#define VULKAN 100
// #extension GL_KHR_vulkan_glsl : enable

layout(location = 0) in VS_OUT { vec2 uv; }
fs_in;

layout(input_attachment_index = 0, set = 0,
       binding = 0) uniform subpassInputMS colorAttachment;
layout(input_attachment_index = 1, set = 0,
       binding = 1) uniform subpassInputMS depthAttachment;

layout(location = 0) out vec4 fragColor;

void main() {
  vec4 color = subpassLoad(colorAttachment, gl_SampleID);
  float depth = subpassLoad(depthAttachment, gl_SampleID).r;

  if (fs_in.uv.x > 0.5) {
    fragColor = vec4(vec3((1.0 - depth) * 255.0), 1.0);
  } else {
    fragColor = color;
  }
}
