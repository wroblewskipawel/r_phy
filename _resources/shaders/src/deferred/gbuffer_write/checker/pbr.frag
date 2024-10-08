#version 460 core

#define VULKAN 100

layout(location = 0) in VS_OUT {
  vec3 pos;
  vec3 norm;
  vec3 color;
  vec2 uv;
}
fs_in;

layout(location = 0) out vec4 gAlbedo;
layout(location = 1) out vec4 gNormal;
layout(location = 2) out vec4 gPosition;

const float CHECKER_SIZE = 3.0;

void main() {
  gNormal = vec4(fs_in.norm, 1.0);
  gPosition = vec4(fs_in.pos, 1.0);
  vec2 signed_uvs = fract(fs_in.uv * CHECKER_SIZE) - 0.5;
  float color_factor = signed_uvs.x * signed_uvs.y > 0.0 ? 0.5 : 1.0;
  gAlbedo = vec4(fs_in.color * color_factor, 1.0);
}
