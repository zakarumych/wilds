
#extension GL_EXT_nonuniform_qualifier : enable

vec4 rand(uvec3 co) {
    float x = fract(sin(dot(co, vec3(12.9898, 78.233, 42.113))) * 4758.5453);
    float y = fract(sin(dot(co, vec3(17.9898, 78.233, 42.113))) * 4358.5453);
    float z = fract(sin(dot(co, vec3(23.9898, 78.233, 42.113))) * 4378.5453);
    float w = fract(sin(dot(co, vec3(27.9898, 78.233, 42.113))) * 4375.5453);
    return vec4(x, y, z, w);
}

vec3 golden_rand(uvec3 co) {
    vec4 rand = rand(co);
    float x = fract(dot(co, vec3(M_FI, 0, M_PI)));
    float y = fract(dot(co, vec3(0, M_PI, M_FI)));
    float z = fract(dot(co, vec3(M_PI, 0, M_FI)));
    return vec3(x, y, z);
}

uint diff(uint a, uint b) {
    return max(a,b) - min(a,b);
}

vec3 blue_rand(uvec3 co) {
    uint x = (co.z % 2 == 0 ? co.x : co.y) % 64;
    uint y = (co.z % 2 == 0 ? co.y : co.x) % 64;

    uint z = ((co.x / 64 + co.y / 64 + co.z) * 2654435761) % 64;
    uint index = x + y * 64 + z * 64 * 64;
    vec4 raw = blue_noise[index];

    if (co.z % 2 == 0)
        raw = raw.yxzw;

    if (co.z % 3 == 0)
        raw = raw.xzyw;

    if (co.z % 5 == 0)
        raw = raw.wyxz;

    if (co.z % 7 == 0)
        raw = raw.zwyx;

    return raw.xyz;
    // return rand(co);
    // return golden_rand(co);
}

vec2 blue_rand_circle(uvec3 co) {
    vec3 rand = golden_rand(co);
    float t = rand.x * 2 * M_PI;
    float u = rand.y * rand.z;
    float r = u > 1 ? 2 - u : u;
    return vec2(r * cos(t), r * sin(t));
}

vec3 blue_rand_cone(uvec3 co, float cos_theta) {
    vec3 rand = golden_rand(co);
    float cos_a = (1 - rand.x) + rand.x * cos_theta;
    float sin_a = sqrt(1 - cos_a * cos_a);
    float phi = rand.y * 2 * M_PI;
    return vec3(cos(phi) * sin_a, sin(phi) * sin_a, cos_a);
}

vec2 blue_rand_square(uvec3 co) {
    return golden_rand(co).xy;
}

vec3 blue_rand_hemisphere_cosine(uvec3 co) {
    vec3 rand = golden_rand(co);
    float a = rand.x * 2 * M_PI;
    float r = sqrt(rand.y);
    float z = sqrt(1 - rand.y);
    return vec3(r*cos(a), r*sin(a), z);
}
