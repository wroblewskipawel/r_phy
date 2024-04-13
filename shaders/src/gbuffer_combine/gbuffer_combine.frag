#version 460 core

#define VULKAN 100

layout(input_attachment_index = 0, set = 0, binding = 0) uniform subpassInputMS gAlbedo;
layout(input_attachment_index = 1, set = 0, binding = 1) uniform subpassInputMS gNormal;
layout(input_attachment_index = 2, set = 0, binding = 2) uniform subpassInputMS gPosition;
layout(input_attachment_index = 3, set = 0, binding = 3) uniform subpassInputMS gDepth;

layout(location = 0) out vec4 fragColor;

void main() {
    vec4 albedo = subpassLoad(gAlbedo, gl_SampleID);
    vec4 normal = subpassLoad(gNormal, gl_SampleID);
    vec4 position = subpassLoad(gPosition, gl_SampleID);
    vec4 depth = subpassLoad(gDepth, gl_SampleID);

    // Do some lighting calculations here
    // For now, just output the albedo
    fragColor = albedo;
 }
