#version 460
#extension GL_EXT_ray_tracing : require
#extension GL_GOOGLE_include_directive : enable
#include "raycommon.glsl"

layout(location = 0) rayPayloadInEXT PrimaryHitPayload prd;

void main() {
    const vec3 emissive = vec3(1.0, 4.0, 7.0);
    const vec3 color = emissive / (emissive + vec3(1, 1, 1));
    prd.albedo = vec4(color, 1.0);
    prd.emissive = emissive;
    prd.direct = 0;
}
