#version 460
#define RAY_TRACING
#extension GL_EXT_ray_tracing : require
#extension GL_GOOGLE_include_directive : enable
#extension GL_EXT_scalar_block_layout : enable

#include "descriptors.glsl"
#include "../common/rayhit.glsl"
#include "../common/rand.glsl"
#include "../common/pbr.glsl"

layout(location = 0) rayPayloadInEXT DiffuseHitPayload prd;
layout(location = 1) rayPayloadEXT uint unshadows;

hitAttributeEXT vec2 attribs;

void main() {
    const vec3 back = gl_WorldRayDirectionEXT * 0.001;
    const uint shadow_rays = 1;
    Rng rng;

    derive_ray_rng(prd.rng_start, rng);

    const uint shadow_ray_flags = gl_RayFlagsOpaqueEXT | gl_RayFlagsTerminateOnFirstHitEXT | gl_RayFlagsSkipClosestHitShaderEXT;
    const vec3 barycentrics = vec3(1.0f - attribs.x - attribs.y, attribs.x, attribs.y);
    uvec3 indices = instance_triangle_indices();

    Vertex v0 = instance_vertex(indices.x);
    Vertex v1 = instance_vertex(indices.y);
    Vertex v2 = instance_vertex(indices.z);

    vec3 pos = v0.pos * barycentrics.x + v1.pos * barycentrics.y + v2.pos * barycentrics.z;
    vec2 uv = v0.uv * barycentrics.x + v1.uv * barycentrics.y + v2.uv * barycentrics.z;

    vec3 normal = normalize(v0.norm * barycentrics.x + v1.norm * barycentrics.y + v2.norm * barycentrics.z);
    vec4 tangh = v0.tangh * barycentrics.x + v1.tangh * barycentrics.y + v2.tangh * barycentrics.z;

    mat3 world_tangent_space = mat3(gl_ObjectToWorldEXT) * tangent_space(normal, tangh);
    vec3 world_normal = sample_normal(world_tangent_space, uv);
    world_normal *= gl_HitKindEXT == gl_HitKindFrontFacingTriangleEXT ? 1 : -1;

    vec3 world_pos = (gl_ObjectToWorldEXT * vec4(pos, 1.0));

    vec3 albedo = sample_albedo(uv).xyz;
    vec2 metalness_roughness = sample_metalness_roughness(uv);
    float metalness = metalness_roughness.x;
    float roughness = metalness_roughness.y;

    vec3 radiation = vec3(0);
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
                traceRayEXT(tlas, shadow_ray_flags, 0xff, 0, 0, 2, world_pos - back, 0, r, 1000.0, 1);
            }

            vec3 rayggx = ggx(world_normal, tolight_dir, -gl_WorldRayDirectionEXT, albedo.rgb, metalness, roughness);
            radiation += globals.dirlight.rad * rayggx / shadow_rays * unshadows;
        }
    }

    // for (uint i = 0; i < globals.plights; ++i)
    // {
    //     if (dot(plight[i].rad, vec3(1, 1, 1)) > 0.0001)
    //     {
    //         vec3 tolight = plight[i].pos - world_pos;
    //         float attenuation = dot(normalize(tolight), world_normal);
    //         if (attenuation > 0.0)
    //         {
    //             float ls = dot(tolight, tolight);
    //             float l = sqrt(ls);
    //             float ray_contribution = attenuation / shadow_rays / ls;

    //             unshadows = 0;
    //             for (int i = 0; i < shadow_rays; ++i)
    //             {
    //                 vec3 r = normalize(rand_sphere(blue_rand(rng)) + tolight);
    //                 traceRayEXT(tlas, shadow_ray_flags, 0xff, 0, 0, 2, world_pos - back, 0, r, l, 1);
    //             }
    //             radiation += plight[i].rad * (ray_contribution * unshadows);
    //         }
    //     }
    // }

    unshadows = 0;
    for (uint i = 0; i < shadow_rays; ++i)
    {
        vec3 d = rand_sphere(blue_rand(rng));
        vec3 r = reflect(gl_WorldRayDirectionEXT, world_normal);
        vec3 dir = normalize(mix(r, d, roughness));
        traceRayEXT(tlas, shadow_ray_flags, 0xff, 0, 0, 2, world_pos - back, 0, dir, 1000.0, 1);
    }
    prd.radiation = radiation + sample_emissive(uv) + globals.skylight * albedo * unshadows / shadow_rays;
}
