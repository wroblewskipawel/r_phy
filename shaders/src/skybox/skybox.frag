#version 460 core

layout(location = 0) in struct VS_OUT {
  vec3 pos_lh;
} fs_in;

layout(location = 0) out vec4 frag_color;

layout(set = 1, binding = 0) uniform samplerCube skybox;

void main() {
  frag_color = texture(skybox, fs_in.pos_lh);
}
