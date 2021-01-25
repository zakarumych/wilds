#version 460

layout(location = 0) in vec3 normal;
layout(location = 1) in vec2 uv;

layout(location = 0) out vec4 out_color;


void main() {
    out_color = vec4(normal, 1.);
}
