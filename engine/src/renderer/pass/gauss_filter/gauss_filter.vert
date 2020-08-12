#version 460

const vec2 triangle[3] = {
    vec2(-1, -1),
    vec2(-1, 3),
    vec2(3, -1),
};

void main() {
    gl_Position = vec4(triangle[gl_VertexIndex], 0, 0);
}
