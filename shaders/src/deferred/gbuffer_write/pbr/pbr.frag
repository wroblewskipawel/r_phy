#version 460 core

#define VULKAN 100

layout(location = 0) in VS_OUT {
    vec3 pos;
    vec3 norm;
    vec2 uv;
} fs_in;

layout(location = 0) out vec4 gAlbedo;
layout(location = 1) out vec4 gNormal;
layout(location = 2) out vec4 gPosition;

const uint ALBEDO_SAMPLER_INDEX = 0;
const uint NORMAL_SAMPLER_INDEX = 1;
const uint METALIC_ROUGHNESS_SAMPLER_INDEX = 2;
const uint OCCLUSION_SAMPLER_INDEX = 3;
const uint EMISSIVE_SAMPLER_INDEX = 4;

layout(set = 1, binding = 0) uniform sampler2D pbrSamplers[5];

void main() {
    gNormal = vec4(fs_in.norm, 1.0);
    gPosition = vec4(fs_in.pos, 1.0);
    gAlbedo = texture(pbrSamplers[ALBEDO_SAMPLER_INDEX], fs_in.uv);;
}
