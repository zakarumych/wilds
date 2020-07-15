
#extension GL_EXT_nonuniform_qualifier : enable

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
