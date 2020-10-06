#version 460
#extension GL_EXT_ray_tracing : require
#extension GL_GOOGLE_include_directive : enable

#include "descriptors.glsl"

layout(location = 0) rayPayloadInEXT DiffuseHitPayload prd;

void main() {
    const vec3 emissive = globals.skylight;
    prd.radiation += emissive;
}
