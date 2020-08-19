
#extension GL_EXT_nonuniform_qualifier : enable

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
    // return rand(co);

    // const double fi3 = 1.2207440846057594753616853491088319144324890862486352142882444530497100085225914;
    // const double i1fi3 = fi3 * fi3 - 1;
    // const double i2fi3 = fi3 - i1fi3;
    // const double i3fi3 = 1 - i2fi3;

    // const vec3 alpha = vec3(i1fi3, i2fi3, i3fi3);
    // float x = wave(0.5 + dot(co, alpha));
    // float y = wave(0.5 + dot(co, alpha.yzx));
    // float z = wave(0.5 + dot(co, alpha.zxy));
    
    const double fi4 = 1.1673039782614186842560458998548421807205603715254890391400824492756519034295270;
    const double i1fi4 = fi4 * fi4 * fi4 - 1;
    const double i2fi4 = fi4 * fi4 - i1fi4;
    const double i3fi4 = fi4 - i2fi4;
    const double i4fi4 = 1 - i3fi4;

    const vec4 alpha = vec4(i1fi4, i2fi4, i3fi4, i4fi4);
    float x = wave(0.5 + dot(co, alpha));
    float y = wave(0.5 + dot(co, alpha.yzwx));
    float z = wave(0.5 + dot(co, alpha.zwxy));

    // // const double fi5 = 1.1347241384015194926054460545064728402796672263828014859251495516682368939998426;
    // // const double i1fi5 = fi5 * fi5 * fi5 * fi5 - 1;
    // // const double i2fi5 = fi5 * fi5 * fi5 - i1fi5;
    // // const double i3fi5 = fi5 * fi5 - i2fi5;
    // // const double i4fi5 = fi5 - i3fi5;
    // // const double i5fi5 = 1 - i4fi5;

    // // const vec3 alpha = vec3(i1fi3, i1fi4, i1fi5);
    // // const vec3 beta = vec3(i2fi3, i2fi4, i2fi5);
    // // const vec3 gamma = vec3(i3fi3, i3fi4, i3fi5);

    // // vec3 fco = vec3(co);
    // // return wave(vec3(dot(fco, alpha), dot(fco, beta), dot(fco, gamma)));

    // // const vec3 alpha = vec3(1/M_PL, 1/M_PL/M_PL, 1/M_PL/M_PL/M_PL);
    // // const vec3 beta = vec3(1/M_FI, 1/M_FI/M_FI, 1/M_FI/M_FI/M_FI);
    // // const vec3 gamma = vec3(1/M_PX, 1/M_PX/M_PX, 1/M_PX/M_PX/M_PX);
    // // float x = wave(0.5 + dot(co, alpha));
    // // float y = wave(0.5 + dot(co, beta));
    // // float z = wave(0.5 + dot(co, gamma));

    // // const vec3 alpha = vec3(0.819172513396164439699571188342427040348497832553712965667, 0.6287067210378086337748232573780154909680339260213870955039718150, 0.6180339887498948482045868343656381177203091798057628621354486227);
    // // const vec3 beta = vec3(0.671043606703789208416815654036199702552744474771178058743, 0.8566748838545028748523248153124343698313999454937526255764128103, 0.3819660112501051517954131656343618822796908201942371378645513772);
    // // const vec3 gamma = vec3(0.549700477901970266944869695072632211879744611477457155545, 0.7338918566271259904047331700024405296994329007761294712589367538, 0.2360679774997896964091736687312762354406183596115257242708972454);
    // // float x = fract(0.5 + dot(co, alpha));
    // // float y = fract(0.5 + dot(co, beta));
    // // float z = fract(0.5 + dot(co, gamma));

    // // const vec3 alpha = vec3(1/fi_4, 1/fi_4/fi_4, 1/fi_4/fi_4/fi_4);
    // // const vec3 beta = vec3(1/fi_4/fi_4, 1/fi_4/fi_4/fi_4, 1/fi_4/fi_4/fi_4/fi_4);
    // // const vec3 gamma = vec3(1/fi_4/fi_4/fi_4, 1/fi_4/fi_4/fi_4/fi_4, 1/fi_4/fi_4/fi_4/fi_4/fi_4);
    // // float x = wave(0.5 + dot(co, alpha));
    // // float y = wave(0.5 + dot(co, beta));
    // // float z = wave(0.5 + dot(co, gamma));
    vec3 v = vec3(x, y, z);

    return v;
}

uint diff(uint a, uint b) {
    return max(a,b) - min(a,b);
}

// vec3 blue_rand(uvec4 co) {
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


float components_sum(vec3 v) {
    return v.x + v.y + v.z;
}
