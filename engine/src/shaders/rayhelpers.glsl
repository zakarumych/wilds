
#extension GL_EXT_nonuniform_qualifier : enable

uvec3 instance_triangle_indices(uint instance, uint primitive) {
  uint mesh = scene.i[instance].mesh;
  return uvec3(indices[mesh].i[3 * primitive + 0],
               indices[mesh].i[3 * primitive + 1],
               indices[mesh].i[3 * primitive + 2]);
}

Vertex instance_vertex(uint instance, uint index) {
  uint mesh = scene.i[instance].mesh;
  return vertices[mesh].v[index];
}

mat4 instance_transform(uint instance) {
  return scene.i[instance].transform;
}

float rand_m(vec2 co) {
    return fract(sin(dot(co.xy, vec2(12.9898, 78.233)) + globals.seconds) * 43758.5453);
}

float rand(vec2 co) {
    return fract(sin(dot(co.xy, vec2(12.9898, 78.233))) * 43758.5453);
}

vec2 rand_c(vec2 co) {
    float t = 2 * M_PI * rand(co);
    float u = rand(co * 2) + rand(co - vec2(1, 2));
    float r = u > 1 ? 2 - u : u;
    return vec2(r * cos(t), r * sin(t));
}

vec2 rand_c_m(vec2 co) {
    float t = 2 * M_PI * rand_m(co);
    float u = rand_m(co * 2) + rand_m(co - vec2(1, 2));
    float r = u > 1 ? 2 - u : u;
    return vec2(r * cos(t), r * sin(t));
}

uint diff(uint a, uint b)
{
    return max(a,b) - min(a,b);
}

float blue_rand(float co) {
  return float(fract(M_FI * co));
}

vec2 blue_rand_c(vec2 co) {
    float t = 2 * M_PI * blue_rand(co.x);
    float u = blue_rand(co.y) + blue_rand(co.x);
    float r = u > 1 ? 2 - u : u;
    return vec2(r * cos(t), r * sin(t));
}

vec3 blue_rand_unit_vector(vec2 co) {
    float a = blue_rand(co.x * 1234.56789) * 2 * M_PI;
    float z = blue_rand(co.y * 1234.56789) * 2 - 1;
    float r = sqrt(1 - z*z);
    return vec3(r*cos(a), r*sin(a), z);
}
