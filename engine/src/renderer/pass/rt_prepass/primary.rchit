#version 460
#define RAY_TRACING
#extension GL_EXT_ray_tracing : require
#extension GL_GOOGLE_include_directive : enable
#extension GL_EXT_scalar_block_layout : enable

#include "descriptors.glsl"
#include "../common/rayhit.glsl"
#include "../common/rand.glsl"
#include "../common/pbr.glsl"
#include "../common/const.glsl"

layout(location = 0) rayPayloadInEXT PrimaryHitPayload prd;
layout(location = 1) rayPayloadEXT uint unshadows;
layout(location = 2) rayPayloadEXT DiffuseHitPayload dprd;

hitAttributeEXT vec2 attribs;

vec3 ggx_sample(mat3 tang, vec3 v, vec3 pos, vec3 albedo, float metalness, float roughness, inout Rng rng) {
    float roughness2 = roughness*roughness;

    vec3 n = tang[2];
    float nv = dot(n, v);
    if (nv <= 0.0) return vec3(0);

    // if (metalness > 0.99 && roughness < 0.01) {
    //     vec3 l = reflect(-v, n);
    //     vec3 F = schlickFresnel(albedo, nv);

    //     dprd.rng_start = rng.index;
    //     traceRayEXT(tlas, 00, 0xff, 1, 0, 1, pos, 0, l, 1000.0, 2);
    //     return dprd.radiation * F;
    // } else {
        vec3 result = vec3(0);
        for (uint i = 0; i < globals.diffuse_rays; ++i) {
            vec4 rand = blue_rand(rng);

            float Phi = 2.0*PI*rand.x;
            float cosThetha = clamp(sqrt( (1.0 - rand.y) / (1.0 + roughness2 * roughness2 * rand.y - rand.y) ), 0, 1);
            float sinThetha = sqrt( 1.0 - cosThetha*cosThetha);
            vec3 h = tang * vec3(sinThetha*cos(Phi), sinThetha*sin(Phi), cosThetha);

            // vec3 l = 2 * dot(h, v) - v;
            vec3 l = reflect(-v, h);
            float nl = dot(n, l);
            if (nl <= 0.0)
                continue;

            float nh = clamp(dot(n, h), 0.0, 1.0);
            float hv = clamp(dot(h, v), 0.0, 1.0);
            // float lh = clamp(dot(l, h), 0.0, 1.0);

            vec3 rayggx;
            vec3 f0 = mix(vec3(0.00, 0.00, 0.00), albedo, metalness);
            vec3 F = schlickFresnel(f0, hv);
            float Favg = (F.x + F.y + F.z) / 3.0;
            float dielectricness = 1.0 - metalness;
            float G = GGXMaskingShadowing(nl, nv, roughness2);
            vec4 rand_filter = blue_rand(rng);
            float dice = rand_filter.x * (Favg * G + dielectricness);
            if ((Favg * G) >= dice) {
                rayggx = F * G / Favg * 0.25 / nv;
            } else {
                l = rand_hemisphere_cosine_dir(blue_rand(rng), n);
                rayggx = albedo  * (1 - F) / (1 - Favg);
            }

            dprd.rng_start = rng.index;
            traceRayEXT(tlas, 00, 0xff, 1, 0, 1, pos, 0, l, 1000.0, 2);
            result += dprd.radiation * rayggx;
        }
        return result / globals.diffuse_rays;
    // }
}

void main()
{
    const vec3 step_back = normalize(gl_WorldRayDirectionEXT) * 0.01;
    Rng rng;
    init_ray_rng(rng);

    float swap_normal = gl_HitKindEXT == gl_HitKindFrontFacingTriangleEXT ? 1 : -1;

    uint shadow_rays = globals.shadow_rays;
    uint diffuse_rays = globals.diffuse_rays;

    const uint shadow_ray_flags = gl_RayFlagsOpaqueEXT | gl_RayFlagsTerminateOnFirstHitEXT | gl_RayFlagsSkipClosestHitShaderEXT;
    const vec3 barycentrics = vec3(1.0f - attribs.x - attribs.y, attribs.x, attribs.y);
    uvec3 indices = instance_triangle_indices();

    Vertex v0 = instance_vertex(indices.x);
    Vertex v1 = instance_vertex(indices.y);
    Vertex v2 = instance_vertex(indices.z);

    vec3 pos = v0.pos * barycentrics.x + v1.pos * barycentrics.y + v2.pos * barycentrics.z;
    vec2 uv = v0.uv * barycentrics.x + v1.uv * barycentrics.y + v2.uv * barycentrics.z;

    vec3 world_pos = (gl_ObjectToWorldEXT * vec4(pos, 1.0));
    vec3 ray_reflect_pos = world_pos - step_back;
    vec3 normal = normalize(v0.norm * barycentrics.x + v1.norm * barycentrics.y + v2.norm * barycentrics.z);
    // normal *= swap_normal;
    vec4 tangh = v0.tangh * barycentrics.x + v1.tangh * barycentrics.y + v2.tangh * barycentrics.z;
    // tangh.w *= swap_normal;

    mat3 world_tangent_space = mat3(gl_ObjectToWorldEXT) * tangent_space(normal, tangh);

    vec3 world_normal = sample_normal(world_tangent_space, uv);

    vec4 albedo = sample_albedo(uv);

    prd.albedo = albedo;
    prd.normal = world_normal;
    prd.depth = gl_HitTEXT;

    vec2 metalness_roughness = sample_metalness_roughness(uv);
    float metalness = metalness_roughness.x;
    float roughness = metalness_roughness.y;

    if (dot(globals.dirlight.rad, vec3(1, 1, 1)) > 0.0001)
    {
        vec3 tolight = -globals.dirlight.dir;
        vec3 tolight_dir = normalize(tolight);
        float attenuation = dot(tolight, world_normal);
        if (attenuation > 0.0)
        {
            unshadows = 0;
            for (uint i = 0; i < shadow_rays; ++i)
            {
                vec3 r = normalize(rand_sphere(blue_rand(rng)) + tolight);
                traceRayEXT(tlas, shadow_ray_flags, 0xff, 0, 0, 2, ray_reflect_pos, 0, r, 1000.0, 1);
            }

            vec3 rayggx = ggx(world_normal, tolight_dir, -gl_WorldRayDirectionEXT, albedo.rgb, metalness, roughness);
            // rayggx = clamp(rayggx, vec3(0), vec3(1));
            prd.direct += globals.dirlight.rad * rayggx / shadow_rays * unshadows;
        }
    }

    // for (uint i = 0; i < globals.plights; ++i)
    // {
    //     if (dot(plight[i].rad, vec3(1, 1, 1)) > 0.0001)
    //     {
    //         vec3 tolight = plight[i].pos - world_pos;
    //         vec3 tolight_dir = normalize(tolight);
    //         float attenuation = dot(normalize(tolight), world_normal);
    //         if (attenuation > 0.0)
    //         {
    //             float ls = dot(tolight, tolight);
    //             float l = sqrt(ls);
    //             float ray_contribution = attenuation / shadow_rays / ls;

    //             unshadows = 0;
    //             for (int i = 0; i < shadow_rays; ++i)
    //             {
    //                 vec3 r = normalize(rand_sphere(blue_rand(rng) + tolight);
    //                 traceRayEXT(tlas, shadow_ray_flags, 0xff, 0, 0, 2, ray_reflect_pos, 0, r, l, 1);
    //             }

    //             vec3 rayggx = ggx(prd.normal, tolight_dir, -gl_WorldRayDirectionEXT, prd.albedo.rgb, metalness, roughness);
    //             prd.direct += rayggx * plight[i].rad * (ray_contribution * unshadows);
    //         }
    //     }
    // }

    // if (prd.ab) {
        prd.diffuse = ggx_sample(world_tangent_space, -gl_WorldRayDirectionEXT, ray_reflect_pos, albedo.rgb, metalness, roughness, rng);
    // } else {
    //     for (uint i = 0; i < diffuse_rays; ++i)
    //     {
    //         vec3 rand = blue_rand(rng);
    //         vec3 dir = rand_hemisphere_cosine_dir(rand, world_normal);
    //         vec3 tolight_dir = normalize(mix(dir, mix(reflected, dir, roughness * roughness), metalness));
    //         vec3 rayggx = ggx(world_normal, tolight_dir, -gl_WorldRayDirectionEXT, albedo.rgb, metalness, roughness);
    //         dprd.radiation = vec3(0, 0, 0);
    //         dprd.ray_index++;
    //         traceRayEXT(tlas, 00, 0xff, 1, 0, 1, ray_reflect_pos, 0, tolight_dir, 1000.0, 2);
    //         prd.diffuse += dprd.radiation * rayggx;
    //     }
    // }
    prd.emissive += sample_emissive(uv);
}
