#version 460 core

#define VULKAN 100

layout(location = 0) in VS_OUT {
    vec3 pos;
    vec3 normal;
    vec2 uv;
} fs_in;

layout(location = 0) out vec4 gAlbedo;
layout(location = 1) out vec4 gNormal;
layout(location = 2) out vec4 gPosition;

layout(set = 1, binding = 0) uniform sampler2D albedoMap;

void main() {
    vec4 albedo = texture(albedoMap, fs_in.uv);
    gAlbedo = albedo;
    gNormal = vec4(fs_in.normal, 1.0);
    gPosition = vec4(fs_in.pos, 1.0);
}
