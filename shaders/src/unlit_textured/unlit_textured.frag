#version 460 core
#define VULKAN 100

layout(location = 0) in struct VS_OUT {
  vec3 color;
  vec2 uv;
} fs_in;

layout(location = 0) out vec4 frag_color;

layout(set = 1, binding = 0) uniform sampler2D albedo_texture;

const float CHECKER_SIZE = 3.0;

void main() { frag_color = frag_color = texture(albedo_texture, fs_in.uv); }
