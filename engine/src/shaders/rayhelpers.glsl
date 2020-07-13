
#extension GL_EXT_nonuniform_qualifier : enable

uvec3 instance_triangle_indices(uint instance, uint primitive) {
    uint mesh = instances[instance].mesh;
    return uvec3(indices[mesh].i[3 * primitive + 0],
                 indices[mesh].i[3 * primitive + 1],
                 indices[mesh].i[3 * primitive + 2]);
}

Vertex instance_vertex(uint instance, uint index) {
    uint mesh = instances[instance].mesh;
    return vertices[mesh].v[index];
}

mat4 instance_transform(uint instance) {
    return instances[instance].transform;
}

vec4 sample_albedo(uint instance, vec2 uv) {
    uint sampler_index = instances[instance].albedo_sampler;
    vec4 raw = vec4(1, 1, 1, 1);
    if (sampler_index > 0)
    {
        raw = texture(albedo[sampler_index-1], uv);
    }
    return raw * instances[instance].albedo_factor;
}

vec3 sample_normal(uint instance, vec2 uv) {
    uint sampler_index = instances[instance].normals_sampler;
    vec3 raw = vec3(0, 0, 1);
    if (sampler_index > 0)
    {
        raw = texture(normals[sampler_index-1], uv).xyz;
    }
    return vec3(raw.xy * instances[instance].normals_factor, raw.z);
}

float rand(uvec3 co) {
    return fract(sin(dot(co, vec3(12.9898, 78.233, 42.113))) * 43758.5453);
}

vec2 rand_circle(uvec3 co) {
    float t = 2 * M_PI * rand(co);
    float u = rand(co * 2) + rand(co + uvec3(1, 2, 3));
    float r = u > 1 ? 2 - u : u;
    return vec2(r * cos(t), r * sin(t));
}

vec3 rand_unit_vector(uvec3 co) {
    float a = rand(co) * 2 * M_PI;
    float z = rand(co + uvec3(1, 2, 3)) * 2 - 1;
    float r = sqrt(1 - z*z);
    return vec3(r*cos(a), r*sin(a), z);
}

uint diff(uint a, uint b) {
    return max(a,b) - min(a,b);
}

vec4 blue_rand_sample(uvec3 co) {
    uint x = (co.z % 2 == 0 ? co.x : co.y) % 64;
    uint y = (co.z % 2 == 0 ? co.y : co.x) % 64;

    uint z = ((co.x / 64 + co.y / 64 + co.z) * 2654435761) % 64;
    uint index = x + y * 64 + z * 64 * 64;
    vec4 raw = blue_noise[index];

    if (co.z % 3 == 0)
        raw = raw.yxzw;

    if (co.z % 5 == 0)
        raw = raw.xzyw;

    if (co.z % 7 == 0)
        raw = raw.xywz;

    if (co.z % 11 == 0)
        raw = raw.zywx;

    return raw + vec4(rand(co), rand(co + uvec3(1, 2, 3)), rand(co + uvec3(2, 3, 4)), rand(co + uvec3(3, 4, 5))) / 5;
}

vec2 blue_rand_circle(uvec3 co) {
    vec4 rand = blue_rand_sample(co);
    float t = rand.x * 2 * M_PI;
    float u = rand.y * rand.z;
    float r = u > 1 ? 2 - u : u;
    return vec2(r * cos(t), r * sin(t));
}

vec2 blue_rand_square(uvec3 co) {
    return blue_rand_sample(co).xy;
}

vec3 blue_rand_unit_vector(uvec3 co) {
    vec4 rand = blue_rand_sample(co);
    float a = rand.x * 2 * M_PI;
    float z = rand.y;
    float r = sqrt(1 - z*z);
    return vec3(r*cos(a), r*sin(a), z);
}
