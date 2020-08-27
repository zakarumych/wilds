#version 460
#extension GL_EXT_ray_tracing : require
#extension GL_GOOGLE_include_directive : enable
#extension GL_EXT_scalar_block_layout : enable

#include "raycommon.glsl"
#include "rayhelpers.glsl"
#include "hithelpers.glsl"


layout(location = 0) rayPayloadInEXT PrimaryHitPayload prd;
layout(location = 1) rayPayloadEXT uint unshadows;
layout(location = 2) rayPayloadEXT DiffuseHitPayload dprd;

hitAttributeEXT vec2 attribs;

void main()
{
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

    vec3 worls_space_pos = (instance_transform() * vec4(pos, 1.0)).xyz;
    vec3 normal = normalize(v0.norm * barycentrics.x + v1.norm * barycentrics.y + v2.norm * barycentrics.z);
    vec4 tangh = v0.tangh * barycentrics.x + v1.tangh * barycentrics.y + v2.tangh * barycentrics.z;
    normal = local_normal(normal, tangh, uv);
    normal *= gl_HitKindEXT == gl_HitKindFrontFacingTriangleEXT ? 1 : -1;
    vec3 world_space_normal = normalize((instance_transform() * vec4(normal, 0.0)).xyz);

    prd.albedo = sample_albedo(uv);
    prd.normal = world_space_normal;
    prd.depth = gl_HitTEXT;

    if (components_sum(globals.dirlight.rad) > 0.0)
    {
        float attenuation = -dot(normalize(globals.dirlight.dir), world_space_normal);
        if (attenuation > 0.0)
        {
            float ray_contribution = attenuation / shadow_rays;

            unshadows = 0;
            for (uint i = 0; i < shadow_rays; ++i)
            {
                vec3 r = normalize(blue_rand_sphere(uvec4(prd.co, i)) - globals.dirlight.dir);
                traceRayEXT(tlas, shadow_ray_flags, 0xff, 0, 0, 2, worls_space_pos, 0.01, r, 1000.0, 1);
            }
            prd.direct += globals.dirlight.rad * (ray_contribution * unshadows);
        }
    }

    for (uint i = 0; i < globals.plights; ++i)
    {
        if (components_sum(plight[i].rad) > 0.0)
        {
            vec3 tolight = plight[i].pos - worls_space_pos;
            float attenuation = dot(normalize(tolight), world_space_normal);
            if (attenuation > 0.0)
            {
                float ls = dot(tolight, tolight);
                float l = sqrt(ls);
                float ray_contribution = attenuation / shadow_rays / ls;

                unshadows = 0;
                for (int i = 0; i < shadow_rays; ++i)
                {
                    vec3 r = normalize(blue_rand_sphere(uvec4(prd.co, i + shadow_rays)) + tolight);
                    traceRayEXT(tlas, shadow_ray_flags, 0xff, 0, 0, 2, worls_space_pos, 0.01, r, l, 1);
                }
                prd.direct += plight[i].rad * (ray_contribution * unshadows);
            }
        }
    }

    dprd.radiation = vec3(0, 0, 0);
    dprd.co = prd.co;
    for (uint i = 0; i < diffuse_rays; ++i)
    {
        dprd.co.z++;
        vec3 dir = blue_rand_hemisphere_cosine_dir(uvec4(prd.co, i + 1024), world_space_normal);
        traceRayEXT(tlas, 00, 0xff, 1, 0, 1, worls_space_pos, 0.001, dir, 1000.0, 2);
    }
    prd.diffuse += dprd.radiation / diffuse_rays;
}
