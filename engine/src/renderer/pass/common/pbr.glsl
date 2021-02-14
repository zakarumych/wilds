#ifndef COMMON_PBR_H
#define COMMON_PBR_H

vec3 schlickFresnel(vec3 R0, float cosTheta) {
    return R0 - (1.0 - R0) * pow(1.0 - clamp(cosTheta, 0.0, 1.0), 5.0);
}

// float GGXMaskingShadowing(float cosThetaN, float alpha) {
//     float cosTheta2 = clamp(cosThetaN * cosThetaN, 0.0, 1.0);
//     float tan2 = ( 1 - cosTheta2 ) / cosTheta2;
//     float GP = 2 / ( 1 + sqrt( 1 + alpha * alpha * tan2 ) );
//     return GP;
// }

float GGXMaskingShadowing(float nl, float nv, float a2)
{
    float a = nv * sqrt(a2 + (1.0 - a2) * nl * nl);
    float b = nl * sqrt(a2 + (1.0 - a2) * nv * nv);

    return 2.0 * nl * nv / (a + b);
}

float distribution(float cosThetaNH, float alpha2) {
    float NH_sqr = clamp(cosThetaNH * cosThetaNH, 0.0, 0.999);
    float den = NH_sqr * alpha2 + (1.0 - NH_sqr);
    return alpha2 / (den * den) * FRAC_1_PI;
}

vec3 ggx(vec3 n, vec3 l, vec3 v, vec3 albedo, float metalness, float roughness) {
    vec3 h = (v + l) / 2.0;
    float nl = dot(n, l);
    if (nl <= 0.0) return vec3(0.0);
    float nv = dot(n, v);
    if (nv <= 0.0) return vec3(0.0);
    float nh = clamp(dot(n, h), 0.0, 1.0);
    float hv = clamp(dot(h, v), 0.0, 1.0);

    // float lh = clamp(dot(l, h), 0.0, 1.0);
    float roughness2 = roughness*roughness;

    float G = GGXMaskingShadowing(nl, nv, roughness2);
    float D = distribution(nh, roughness2);

    vec3 f0 = mix(vec3(0.04, 0.04, 0.04), albedo, metalness);
    vec3 F = schlickFresnel(f0, hv);
    vec3 spec = G*D*F*0.25/nv;
    vec3 diff = (1.0 - metalness) * clamp(vec3(1.0) - F, 0.0, 1.0);

    return max(vec3(0.0), albedo * diff * nl * FRAC_1_PI + spec);
}


#endif
