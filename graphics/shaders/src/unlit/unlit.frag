#version 460 core
#define VULKAN 100

layout(location = 0) in struct VS_OUT {
  vec3 color;
  vec2 uv;
} fs_in;

layout(location = 0) out vec4 frag_color;

const float CHECKER_SIZE = 3.0;

void main() {
  vec2 signed_uvs = fract(fs_in.uv * CHECKER_SIZE) - 0.5;
  float color_factor = signed_uvs.x * signed_uvs.y > 0.0 ? 0.5 : 1.0;
  frag_color = vec4(fs_in.color * color_factor, 1.0);
}
