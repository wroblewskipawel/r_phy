#version 460 core

layout(location = 0) in vec3 pos;
layout(location = 1) in vec3 color;
layout(location = 2) in vec3 norm;
layout(location = 3) in vec2 uv;
layout(location = 4) in vec4 tangent;

layout(location = 0) out struct VS_OUT { vec3 pos_lh; } vs_out;

layout(push_constant) uniform transform {
  mat4 view;
  mat4 proj;
}
c;

void main() {
  vs_out.pos_lh = vec3(pos.x, pos.z, pos.y);
  vec4 pos = c.proj * c.view * vec4(pos, 1.0);
  gl_Position = pos.xyww;
}
