#version 460
#extension GL_EXT_ray_tracing : require
#extension GL_GOOGLE_include_directive : enable
#extension GL_EXT_scalar_block_layout : enable
#include "raycommon.glsl"

layout(location = 0) rayPayloadInEXT PrimaryHitPayload prd;

void main() {
}
