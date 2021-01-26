#version 460

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec3 tangent;
layout(location = 3) in vec2 uv;

layout(location = 4) in mat4x3 model;

layout(location = 0) out vec3 out_normal;
layout(location = 1) out vec2 out_uv;

layout(set = 0, binding = 0) uniform Globals {
    mat4x3 view;
    mat3 proj;
} globals;

void main() {
    mat4x3 view_model = globals.view * mat4(model);

    gl_Position = vec4(globals.proj * globals.view * vec4(model * vec4(position, 1.), 1.), 1.);
    out_normal = globals.view * vec4(model * vec4(normal, 0.), .0);
    out_uv = uv;
}
