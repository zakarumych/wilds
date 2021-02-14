#version 460
#define RAY_TRACING
#extension GL_EXT_ray_tracing : require
#extension GL_GOOGLE_include_directive : enable

#include "descriptors.glsl"

layout(location = 0) rayPayloadInEXT DiffuseHitPayload prd;

void main() {
    prd.radiation = globals.skylight;
}
