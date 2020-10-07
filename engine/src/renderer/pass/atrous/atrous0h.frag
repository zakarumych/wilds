#version 460
#extension GL_GOOGLE_include_directive : enable

layout(binding = 0, set = 0) uniform sampler2D normals_depth;
layout(binding = 1, set = 0) uniform sampler2D unfiltered;

layout(location = 0) out vec4 output_image;

const int h = 0;
const int w = 8;
const int l = 1;

#include "atrous_main.glsl"
