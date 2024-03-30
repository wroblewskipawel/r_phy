#version 460 core

layout(location = 0) in vec3 pos;
layout(location = 1) in vec3 color;
layout(location = 2) in vec3 norm;
layout(location = 3) in vec2 uv;

layout(location=0) out struct VS_OUT {
    vec3 pos_lh;
} vs_out;

layout(set=0, binding=0) uniform camera {
    mat4 view;
    mat4 proj;
} c;

// Not necessarily needed here,
// keep it for now as same `camera` uniform buffer is used
// for both skybox and model rendering piplines
layout(push_constant) uniform transform { mat4 model; }
m;

void main() {
    vs_out.pos_lh = vec3(pos.x, pos.z, pos.y);
    gl_Position = c.proj * c.view * m.model * vec4(pos, 1.0);
}
