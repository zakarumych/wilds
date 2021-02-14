#ifndef COMMON_RAND_H
#define COMMON_RAND_H
#include "const.glsl"

struct Rng {
    uvec3 seed;
    uint index;
};

#ifdef RAY_TRACING
void init_ray_rng(out Rng rng) {
    Rng new = { uvec3(gl_LaunchIDEXT.xy, globals.frame), 0 };
    // Rng new = { uvec3(gl_LaunchIDEXT.xy, 0), 0 };
    rng = new;
}
void derive_ray_rng(uint start, out Rng rng) {
    init_ray_rng(rng);
    rng.seed.z = start * 17;
}
#endif

vec4 blue_rand(inout Rng rng) {
    uint x = rng.seed.x & 255;
    uint y = rng.seed.y & 255;
    uint z = (rng.seed.z + rng.index * 13) & 127;

    uint index = x + y * 256 + z * 256 * 256;
    vec4 raw = blue_noise[index];
    rng.index++;

    return raw;
}


// float wave(float x) {
//     float f = fract(x);
//     return f < 0.5 ? 2 * f : 2 - 2 * f;
// }

// vec2 wave(vec2 v) {
//     return vec2(wave(v.x), wave(v.y));
// }

// vec3 wave(vec3 v) {
//     return vec3(wave(v.x), wave(v.y), wave(v.z));
// }


// vec4 wave(vec4 v) {
//     return vec4(wave(v.x), wave(v.y), wave(v.z), wave(v.w));
// }

// double wave(double x) {
//     double f = fract(x);
//     return f < 0.5 ? 2 * f : 2 - 2 * f;
// }

// dvec2 wave(dvec2 v) {
//     return dvec2(wave(v.x), wave(v.y));
// }

// dvec3 wave(dvec3 v) {
//     return dvec3(wave(v.x), wave(v.y), wave(v.z));
// }

// dvec4 wave(dvec4 v) {
//     return dvec4(wave(v.x), wave(v.y), wave(v.z), wave(v.w));
// }



// vec3 nice_rand(uvec4 co) {
//     // return rand(co);

//     // const double fi3 = 1.2207440846057594753616853491088319144324890862486352142882444530497100085225914;
//     // const double i1fi3 = fi3 * fi3 - 1;
//     // const double i2fi3 = fi3 - i1fi3;
//     // const double i3fi3 = 1 - i2fi3;

//     // const vec3 alpha = vec3(i1fi3, i2fi3, i3fi3);
//     // float x = wave(0.5 + dot(co, alpha));
//     // float y = wave(0.5 + dot(co, alpha.yzx));
//     // float z = wave(0.5 + dot(co, alpha.zxy));
    
//     const double fi4 = 1.1673039782614186842560458998548421807205603715254890391400824492756519034295270;
//     // const double i1fi4 = fi4 * fi4 * fi4 - 1;
//     // const double i2fi4 = fi4 * fi4 - i1fi4;
//     // const double i3fi4 = fi4 - i2fi4;
//     // const double i4fi4 = 1 - i3fi4;

//     // const vec4 alpha = vec4(i1fi4, i2fi4, i3fi4, i4fi4);
//     // float x = wave(0.1 + dot(co, alpha));
//     // float y = wave(0.2 + dot(co, alpha.yzwx));
//     // float z = wave(0.3 + dot(co, alpha.wzyx));

//     // const double fi5 = 1.1347241384015194926054460545064728402796672263828014859251495516682368939998426;
//     // const double i1fi5 = fi5 * fi5 * fi5 * fi5 - 1;
//     // const double i2fi5 = fi5 * fi5 * fi5 - i1fi5;
//     // const double i3fi5 = fi5 * fi5 - i2fi5;
//     // const double i4fi5 = fi5 - i3fi5;
//     // const double i5fi5 = 1 - i4fi5;

//     // const vec3 alpha = vec3(i1fi3, i1fi4, i1fi5);
//     // const vec3 beta = vec3(i2fi3, i2fi4, i2fi5);
//     // const vec3 gamma = vec3(i3fi3, i3fi4, i3fi5);

//     // vec3 fco = vec3(co);
//     // return wave(vec3(dot(fco, alpha), dot(fco, beta), dot(fco, gamma)));

//     // const vec3 alpha = vec3(1/M_PL, 1/M_PL/M_PL, 1/M_PL/M_PL/M_PL);
//     // const vec3 beta = vec3(1/M_FI, 1/M_FI/M_FI, 1/M_FI/M_FI/M_FI);
//     // const vec3 gamma = vec3(1/M_PX, 1/M_PX/M_PX, 1/M_PX/M_PX/M_PX);
//     // float x = wave(0.5 + dot(co, alpha));
//     // float y = wave(0.5 + dot(co, beta));
//     // float z = wave(0.5 + dot(co, gamma));

//     // const vec3 alpha = vec3(0.819172513396164439699571188342427040348497832553712965667, 0.6287067210378086337748232573780154909680339260213870955039718150, 0.6180339887498948482045868343656381177203091798057628621354486227);
//     // const vec3 beta = vec3(0.671043606703789208416815654036199702552744474771178058743, 0.8566748838545028748523248153124343698313999454937526255764128103, 0.3819660112501051517954131656343618822796908201942371378645513772);
//     // const vec3 gamma = vec3(0.549700477901970266944869695072632211879744611477457155545, 0.7338918566271259904047331700024405296994329007761294712589367538, 0.2360679774997896964091736687312762354406183596115257242708972454);
//     // float x = fract(0.5 + dot(co, alpha));
//     // float y = fract(0.5 + dot(co, beta));
//     // float z = fract(0.5 + dot(co, gamma));

//     const vec4 alpha = vec4(1/fi4, 1/fi4/fi4, 1/fi4/fi4/fi4, 1/fi4/fi4/fi4/fi4);
//     const vec4 beta = vec4(1/fi4/fi4, 1/fi4/fi4/fi4, 1/fi4/fi4/fi4/fi4, 1/fi4/fi4/fi4/fi4/fi4);
//     const vec4 gamma = vec4(1/fi4/fi4/fi4, 1/fi4/fi4/fi4/fi4, 1/fi4/fi4/fi4/fi4/fi4, 1/fi4/fi4/fi4/fi4/fi4/fi4);
//     float x = wave(0.5 + dot(co, alpha));
//     float y = wave(0.5 + dot(co, beta));
//     float z = wave(0.5 + dot(co, gamma));
//     vec3 v = vec3(x, y, z);

//     return v;
// }


// // vec3 rand(uvec4 co) {
// //     double x = fract(sin(dot(co.xy, vec2(12.9898, 78.233))) * 4758.5453);
// //     double y = fract(sin(dot(co.zw, vec2(3.1415, 11.777))) * 758.5453);
// //     double z = fract(sin(dot(co.yz, vec2(0.111111, 23.9859))) * 14758.5453);
// //     return vec3(x, y, z);
// // }

vec2 rand_circle(vec3 rand) {
    float t = rand.x * 2 * PI;
    float u = rand.y * rand.z;
    float r = u > 1 ? 2 - u : u;
    return vec2(r * cos(t), r * sin(t));
}

vec2 rand_circle(vec4 rand) {
    return rand_circle(rand.xyz);
}

vec2 rand_square(vec2 rand) {
    return rand.xy;
}

vec2 rand_square(vec3 rand) {
    return rand.xy;
}

vec2 rand_square(vec4 rand) {
    return rand.xy;
}

vec3 rand_sphere(vec3 rand) {
    float theta = rand.x * 2 * PI;
    float phi = rand.y * PI;
    float r = rand.z;
    return vec3(r*sin(phi)*cos(theta),r*sin(phi)*sin(theta), r*cos(phi));
}

vec3 rand_sphere(vec4 rand) {
    return rand_sphere(rand.xyz);
}

vec3 rand_hemisphere_cosine(vec2 rand) {
    float x = sqrt(rand.x)*cos(2*PI*rand.y);
    float y = sqrt(rand.x)*sin(2*PI*rand.y);
    float z = sqrt(1 - rand.x);

    return vec3(x, y, z);
}

vec3 rand_hemisphere_cosine(vec3 rand) {
    return rand_hemisphere_cosine(rand.xy);
}

vec3 rand_hemisphere_cosine(vec4 rand) {
    return rand_hemisphere_cosine(rand.xy);
}

vec3 rand_hemisphere_cosine_dir(vec2 rand, vec3 dir) {
    float sin_theta = 0.99 * (1 - 2*rand.x);
    float cos_theta = 0.99 * (sqrt(1 - sin_theta*sin_theta));
    float phi = 2*PI*rand.y;
    float x = dir.x + cos_theta*cos(phi);
    float y = dir.y + cos_theta*sin(phi);
    float z = dir.z + sin_theta;
    return normalize(vec3(x, y, z));
}

vec3 rand_hemisphere_cosine_dir(vec3 rand, vec3 dir) {
    return rand_hemisphere_cosine_dir(rand.xy, dir);
}

vec3 rand_hemisphere_cosine_dir(vec4 rand, vec3 dir) {
    return rand_hemisphere_cosine_dir(rand.xy, dir);
}

vec3 rand_cone(vec2 rand, float cos_theta) {
    float cos_a = rand.x * cos_theta;
    float sin_a = sqrt(1 - cos_a * cos_a);
    float phi = rand.y * 2 * PI;
    float x = cos(phi) * sin_a;
    float y = sin(phi) * sin_a;
    float z = cos_a;
    return vec3(x, y, z);
}

vec3 rand_cone(vec3 rand, float cos_theta) {
    return rand_cone(rand.xy, cos_theta);
}

vec3 rand_cone(vec4 rand, float cos_theta) {
    return rand_cone(rand.xy, cos_theta);
}

vec3 rand_cone_dir(vec2 rand, float cos_theta, vec3 dir) {
    vec3 tang = normalize(max(cross(vec3(1, 0, 0), dir), max(cross(vec3(0, 1, 0), dir), cross(vec3(0, 0, 1), dir))));
    vec3 bitang = cross(dir, tang);

    mat3 rot = mat3(bitang, tang, dir);
    vec3 cone = rand_cone(rand, cos_theta);
    return rot * cone;
}

vec3 rand_cone_dir(vec3 rand, float cos_theta, vec3 dir) {
    return rand_cone_dir(rand.xy, cos_theta, dir);
}

vec3 rand_cone_dir(vec4 rand, float cos_theta, vec3 dir) {
    return rand_cone_dir(rand.xy, cos_theta, dir);
}

#endif
