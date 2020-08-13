
#extension GL_EXT_nonuniform_qualifier : enable

vec4 rand(uvec3 co) {
    float x = fract(sin(dot(co, vec3(12.9898, 78.233, 42.113))) * 4758.5453);
    float y = fract(sin(dot(co, vec3(17.9898, 78.233, 42.113))) * 4358.5453);
    float z = fract(sin(dot(co, vec3(23.9898, 78.233, 42.113))) * 4378.5453);
    float w = fract(sin(dot(co, vec3(27.9898, 78.233, 42.113))) * 4375.5453);
    return vec4(x, y, z, w);
}

vec3 blue_rand(uvec3 co) {
    const vec3 alpha = vec3(1/M_PL, 1/M_FI, 1/M_PX);
    const vec3 beta = vec3(1/M_PL/M_PL, 1/M_FI/M_FI, 1/M_PX/M_PX);
    const vec3 gamma = vec3(1/M_PL/M_PL/M_PL, 1/M_FI/M_FI/M_FI, 1/M_PX/M_PX/M_PX);
    float x = fract(0.5 + dot(co, alpha));
    float y = fract(0.5 + dot(co, beta));
    float z = fract(0.5 + dot(co, gamma));
    vec3 v = vec3(x, y, z);

    // const vec3 alpha = vec3(1/M_PX, 1/M_FI, 1/M_PL);
    // const vec3 beta = vec3(1/M_FI, 1/M_PL, 1/M_PI);
    // const vec3 gamma = vec3(1/M_PL, 1/M_PX, 1/M_FI);
    // float x = fract(0.5 + dot(co, alpha));
    // float y = fract(0.5 + dot(co, beta));
    // float z = fract(0.5 + dot(co, gamma));
    // vec3 v = vec3(x, y, z);

    return v;
}

uint diff(uint a, uint b) {
    return max(a,b) - min(a,b);
}

// vec3 blue_rand(uvec3 co) {
//     uint x = (co.z % 2 == 0 ? co.x : co.y) % 64;
//     uint y = (co.z % 2 == 0 ? co.y : co.x) % 64;

//     uint z = ((co.x / 64 + co.y / 64 + co.z) * 2654435761) % 64;
//     uint index = x + y * 64 + z * 64 * 64;
//     vec4 raw = blue_noise[index];

//     if (co.z % 2 == 0)
//         raw = raw.yxzw;

//     if (co.z % 3 == 0)
//         raw = raw.xzyw;

//     if (co.z % 5 == 0)
//         raw = raw.wyxz;

//     if (co.z % 7 == 0)
//         raw = raw.zwyx;

//     return raw.xyz;
// }

vec2 blue_rand_circle(uvec3 co) {
    vec3 rand = blue_rand(co);
    float t = rand.x * 2 * M_PI;
    float u = rand.y * rand.z;
    float r = u > 1 ? 2 - u : u;
    return vec2(r * cos(t), r * sin(t));
}

vec2 blue_rand_square(uvec3 co) {
    return blue_rand(co).xy;
}

vec3 blue_rand_sphere(uvec3 co) {
    vec3 rand = blue_rand(co);
    float theta = rand.x * 2 * M_PI;
    float phi = rand.y * M_PI;
    float r = rand.z;
    return vec3(r*sin(phi)*cos(theta),r*sin(phi)*sin(theta), r*cos(phi));
}

vec3 blue_rand_hemisphere_cosine(uvec3 co) {
    vec3 rand = blue_rand(co);
    float x = sqrt(rand.x)*cos(2*M_PI*rand.y);
    float y = sqrt(rand.x)*sin(2*M_PI*rand.y);
    float z = sqrt(1 - rand.x);

    return vec3(x, y, z);
}

vec3 blue_rand_hemisphere_cosine_dir(uvec3 co, vec3 dir) {
    vec3 rand = blue_rand(co);
    float sin_theta = 0.99 * (1 - 2*rand.x);
    float cos_theta = 0.99 * (sqrt(1 - sin_theta*sin_theta));
    float phi = 2*M_PI*rand.y;
    float x = dir.x + cos_theta*cos(phi);
    float y = dir.y + cos_theta*sin(phi);
    float z = dir.z + sin_theta;
    return normalize(vec3(x, y, z));
}

vec3 blue_rand_cone(uvec3 co, float cos_theta) {
    vec3 rand = blue_rand(co);
    float cos_a = rand.x * cos_theta;
    float sin_a = sqrt(1 - cos_a * cos_a);
    float phi = rand.y * 2 * M_PI;
    float x = cos(phi) * sin_a;
    float y = sin(phi) * sin_a;
    float z = cos_a;
    return vec3(x, y, z);
}

vec3 blue_rand_cone_dir(uvec3 co, float cos_theta, vec3 dir) {
    // vec3 v = blue_rand_sphere(co);
    // for (int i = 0; dot(v, dir) < cos_theta; ++i) {
    //     v = blue_rand_sphere(co + uvec3(i * 137));
    // }

    // return v;

    vec3 tang = normalize(max(cross(vec3(1, 0, 0), dir), max(cross(vec3(0, 1, 0), dir), cross(vec3(0, 0, 1), dir))));
    vec3 bitang = cross(dir, tang);

    mat3 rot = mat3(bitang, tang, dir);
    vec3 cone = blue_rand_cone(co, cos_theta);
    return rot * cone;
}
