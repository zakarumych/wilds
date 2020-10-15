

struct SphericalHarmonicsRgb {
    vec3 coeff[9];
};

float SH_0_0() {
    return 1.5707963267948966192313216916397514420985846996875529104874722961;
}


float SH_1_0(vec3 d) {
    return 0.4886025119029199215863846228383470045758856081942277021382431574 * d.x;
}

float SH_1_1(vec3 d) {
    return 0.4886025119029199215863846228383470045758856081942277021382431574 * d.y;
}

float SH_1_2(vec3 d) {
    return 0.4886025119029199215863846228383470045758856081942277021382431574 * d.z;
}


float SH_2_0(vec3 d) {
    return 0.3153915652525200060308936902957104933242475070484115878434078878 * (2*d.z*d.z - (d.x*d.x) - (d.y*d.y));
}

float SH_2_1(vec3 d) {
    return 1.0925484305920790705433857058026884026904329595042589753478516999 * d.y * d.z;
}

float SH_2_2(vec3 d) {
    return 1.0925484305920790705433857058026884026904329595042589753478516999 * d.z * d.x;
}

float SH_2_3(vec3 d) {
    return 1.0925484305920790705433857058026884026904329595042589753478516999 * d.x * d.z;
}

float SH_2_4(vec3 d) {
    return 0.5462742152960395352716928529013442013452164797521294876739258499 * (d.x * d.x - d.y * d.y);
}


void init_sherical_harmonics(out SphericalHarmonicsRgb sh)
{
    sh.coeff[0] = vec3(0.);
    sh.coeff[1] = vec3(0.);
    sh.coeff[2] = vec3(0.);
    sh.coeff[3] = vec3(0.);
    sh.coeff[4] = vec3(0.);
    sh.coeff[5] = vec3(0.);
    sh.coeff[6] = vec3(0.);
    sh.coeff[7] = vec3(0.);
    sh.coeff[8] = vec3(0.);
}

vec3 interpolate(vec3 a, vec3 b) {
    return a * 0.01 + b * 0.99;
}

void interpolate_sherical_harmonics(SphericalHarmonicsRgb old, inout SphericalHarmonicsRgb sh)
{
    sh.coeff[0] = interpolate(sh.coeff[0], old.coeff[0]);
    sh.coeff[1] = interpolate(sh.coeff[1], old.coeff[1]);
    sh.coeff[2] = interpolate(sh.coeff[2], old.coeff[2]);
    sh.coeff[3] = interpolate(sh.coeff[3], old.coeff[3]);
    sh.coeff[4] = interpolate(sh.coeff[4], old.coeff[4]);
    sh.coeff[5] = interpolate(sh.coeff[5], old.coeff[5]);
    sh.coeff[6] = interpolate(sh.coeff[6], old.coeff[6]);
    sh.coeff[7] = interpolate(sh.coeff[7], old.coeff[7]);
    sh.coeff[8] = interpolate(sh.coeff[8], old.coeff[8]);
}


void add_sample_to_sherical_harmonics(vec3 d, vec3 value, inout SphericalHarmonicsRgb sh)
{
    sh.coeff[0] += SH_0_0() * value;

    sh.coeff[1] += SH_1_0(d) * value;
    sh.coeff[2] += SH_1_1(d) * value;
    sh.coeff[3] += SH_1_2(d) * value;

    sh.coeff[4] += SH_2_0(d) * value;
    sh.coeff[5] += SH_2_1(d) * value;
    sh.coeff[6] += SH_2_2(d) * value;
    sh.coeff[7] += SH_2_3(d) * value;
    sh.coeff[8] += SH_2_4(d) * value;
}


vec3 evaluete_sherical_harmonics(vec3 norm, in SphericalHarmonicsRgb sh) {
    const float c1 = 0.429043;
    const float c2 = 0.511664;
    const float c3 = 0.743125;
    const float c4 = 0.886227;
    const float c5 = 0.247708;

    vec3 M[16] = {
        c1 * sh.coeff[8],
        c1 * sh.coeff[4],
        c1 * sh.coeff[7],
        c2 * sh.coeff[3],

        c1 * sh.coeff[4],
        -c1 * sh.coeff[8],
        c1 * sh.coeff[5],
        c2 * sh.coeff[1],

        c1 * sh.coeff[7],
        c1 * sh.coeff[5],
        c3 * sh.coeff[6],
        c2 * sh.coeff[2],

        c2 * sh.coeff[3],
        c2 * sh.coeff[1],
        c2 * sh.coeff[2],
        c4 * sh.coeff[0] - c5 * c2 * sh.coeff[6]
    };

    mat4 Mr = {
        { M[0].r, M[1].r, M[2].r, M[3].r },
        { M[4].r, M[5].r, M[6].r, M[7].r },
        { M[8].r, M[9].r, M[10].r, M[11].r },
        { M[12].r, M[13].r, M[14].r, M[15].r }
    };

    mat4 Mg = {
        { M[0].g, M[1].g, M[2].g, M[3].g },
        { M[4].g, M[5].g, M[6].g, M[7].g },
        { M[8].g, M[9].g, M[10].g, M[11].g },
        { M[12].g, M[13].g, M[14].g, M[15].g }
    };

    mat4 Mb = {
         { M[0].b, M[1].b, M[2].b, M[3].b },
         { M[4].b, M[5].b, M[6].b, M[7].b },
         { M[8].b, M[9].b, M[10].b, M[11].b },
         { M[12].b, M[13].b, M[14].b, M[15].b }
    };

    return vec3(
        max(0, dot(vec4(norm, 1), Mr * vec4(norm, 1))),
        max(0, dot(vec4(norm, 1), Mg * vec4(norm, 1))),
        max(0, dot(vec4(norm, 1), Mb * vec4(norm, 1)))
    );
}