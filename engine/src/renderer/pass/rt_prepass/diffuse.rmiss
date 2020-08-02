#version 460
#extension GL_EXT_ray_tracing : require
#extension GL_GOOGLE_include_directive : enable
#include "raycommon.glsl"

layout(location = 0) rayPayloadInEXT DiffuseHitPayload prd;

void main() {
    const vec3 emissive = vec3(1.0, 3.0, 5.0) / 3;
    prd.radiation = emissive;
}
