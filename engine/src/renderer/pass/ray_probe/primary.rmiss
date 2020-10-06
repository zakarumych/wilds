#version 460
#extension GL_EXT_ray_tracing : require
#extension GL_GOOGLE_include_directive : enable

#include "../common/sh.glsl"
#include "descriptors.glsl"

layout(location = 0) rayPayloadInEXT PrimaryHitPayload prd;

void main() {
    prd.result += globals.skylight;
}
