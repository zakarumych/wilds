

#ifndef M_PI
#define M_PI 3.1415926535897932384626433832795
#endif


float wave(float x) {
    float f = fract(x);
    return f < 0.5 ? 2 * f : 2 - 2 * f;
}

vec2 wave(vec2 v) {
    return vec2(wave(v.x), wave(v.y));
}

vec3 wave(vec3 v) {
    return vec3(wave(v.x), wave(v.y), wave(v.z));
}

vec4 wave(vec4 v) {
    return vec4(wave(v.x), wave(v.y), wave(v.z), wave(v.w));
}

double wave(double x) {
    double f = fract(x);
    return f < 0.5 ? 2 * f : 2 - 2 * f;
}

dvec2 wave(dvec2 v) {
    return dvec2(wave(v.x), wave(v.y));
}

dvec3 wave(dvec3 v) {
    return dvec3(wave(v.x), wave(v.y), wave(v.z));
}

dvec4 wave(dvec4 v) {
    return dvec4(wave(v.x), wave(v.y), wave(v.z), wave(v.w));
}


vec3 rand(uvec4 co) {
    double x = fract(sin(dot(co.xy, vec2(12.9898, 78.233))) * 4758.5453);
    double y = fract(sin(dot(co.zw, vec2(12.9898, 78.233))) * 4758.5453);
    double z = fract(sin(dot(co.yz, vec2(12.9898, 78.233))) * 4758.5453);
    return vec3(x, y, z);
}

vec3 blue_rand(uvec4 co) {
    uint x = co.x % 256;
    uint y = co.y % 256;
    uint z = (co.z + co.w) % 128;

    uint index = x + y * 256 + z * 256 * 256;
    vec4 raw = blue_noise[index];

    return raw.xyz;
}

vec2 blue_rand_circle(uvec4 co) {
    vec3 rand = blue_rand(co);
    float t = rand.x * 2 * M_PI;
    float u = rand.y * rand.z;
    float r = u > 1 ? 2 - u : u;
    return vec2(r * cos(t), r * sin(t));
}

vec2 blue_rand_square(uvec4 co) {
    return blue_rand(co).xy;
}

vec3 blue_rand_sphere(uvec4 co) {
    vec3 rand = blue_rand(co);
    float theta = rand.x * 2 * M_PI;
    float phi = rand.y * M_PI;
    float r = rand.z;
    return vec3(r*sin(phi)*cos(theta),r*sin(phi)*sin(theta), r*cos(phi));
}

vec3 blue_rand_hemisphere_cosine(uvec4 co) {
    vec3 rand = blue_rand(co);
    float x = sqrt(rand.x)*cos(2*M_PI*rand.y);
    float y = sqrt(rand.x)*sin(2*M_PI*rand.y);
    float z = sqrt(1 - rand.x);

    return vec3(x, y, z);
}

vec3 blue_rand_hemisphere_cosine_dir(uvec4 co, vec3 dir) {
    vec3 rand = blue_rand(co);
    float sin_theta = 0.99 * (1 - 2*rand.x);
    float cos_theta = 0.99 * (sqrt(1 - sin_theta*sin_theta));
    float phi = 2*M_PI*rand.y;
    float x = dir.x + cos_theta*cos(phi);
    float y = dir.y + cos_theta*sin(phi);
    float z = dir.z + sin_theta;
    return normalize(vec3(x, y, z));
}

vec3 blue_rand_cone(uvec4 co, float cos_theta) {
    vec3 rand = blue_rand(co);
    float cos_a = rand.x * cos_theta;
    float sin_a = sqrt(1 - cos_a * cos_a);
    float phi = rand.y * 2 * M_PI;
    float x = cos(phi) * sin_a;
    float y = sin(phi) * sin_a;
    float z = cos_a;
    return vec3(x, y, z);
}

vec3 blue_rand_cone_dir(uvec4 co, float cos_theta, vec3 dir) {
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
