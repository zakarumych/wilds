#version 460
#extension GL_EXT_ray_tracing : require
#extension GL_GOOGLE_include_directive : enable
#extension GL_EXT_scalar_block_layout : enable

#include "../common/sh.glsl"
#include "descriptors.glsl"
#include "../common/rand.glsl"
#include "../common/rayhit.glsl"

layout(location = 0) rayPayloadInEXT PrimaryHitPayload prd;
layout(location = 1) rayPayloadEXT uint unshadows;

hitAttributeEXT vec2 attribs;

void query_probe(ivec3 probe, vec3 origin, vec3 normal, inout vec3 result, inout float weight)
{
    if (probe.x < 0 || probe.y < 0 || probe.z < 0 || probe.x >= gl_LaunchSizeEXT.x || probe.y >= gl_LaunchSizeEXT.y || probe.z >= gl_LaunchSizeEXT.z)
        return;
    
    const uint probe_index = probe.x + probe.y * gl_LaunchSizeEXT.x + probe.z * gl_LaunchSizeEXT.x * gl_LaunchSizeEXT.y;

    const uint shadow_ray_flags = gl_RayFlagsOpaqueEXT | gl_RayFlagsTerminateOnFirstHitEXT | gl_RayFlagsSkipClosestHitShaderEXT;
    vec3 probe_cell_size = globals.extent / vec3(gl_LaunchSizeEXT);

    vec3 probe_location = probe_cell_size * probe + globals.offset;
    vec3 toprobe = probe_location - origin;
    float dist_squared = dot(toprobe, toprobe);
    float dist = sqrt(dist_squared);
    vec3 dir = toprobe / dist;

    unshadows = 0;
    traceRayEXT(tlas, shadow_ray_flags, 0xff, 0, 0, 2, origin, 0, dir, dist, 1);
    if (unshadows > 0)
    {
        result += evaluete_sherical_harmonics(normal, probes[probe_index].spherical_harmonics);
        weight += 1.0 / dist_squared;
    }
}


void query_probes(vec3 origin, vec3 normal, inout vec3 result, inout float weight)
{
    vec3 probe_cell_size = globals.extent / vec3(gl_LaunchSizeEXT);
    vec3 loc = origin - globals.offset / probe_cell_size;
    ivec3 probe;

    probe = ivec3(floor(loc));
    query_probe(probe, origin, normal, result, weight);
    probe = ivec3(floor(loc.xy), ceil(loc.z));
    query_probe(probe, origin, normal, result, weight);
    probe = ivec3(floor(loc.x), ceil(loc.y), floor(loc.z));
    query_probe(probe, origin, normal, result, weight);
    probe = ivec3(floor(loc.x), ceil(loc.yz));
    query_probe(probe, origin, normal, result, weight);
    probe = ivec3(ceil(loc.x), floor(loc.yz));
    query_probe(probe, origin, normal, result, weight);
    probe = ivec3(ceil(loc.x), floor(loc.y), ceil(loc.z));
    query_probe(probe, origin, normal, result, weight);
    probe = ivec3(ceil(loc.xy), floor(loc.z));
    query_probe(probe, origin, normal, result, weight);
    probe = ivec3(ceil(loc));
    query_probe(probe, origin, normal, result, weight);
}

vec3 query_diffuse_from_probes(vec3 origin, vec3 normal)
{
    vec3 result = vec3(0.);
    float weight = 0.;
    query_probes(origin, normal, result, weight);

    return result / weight;
}

void main()
{
    const uvec4 co = uvec4(gl_LaunchIDEXT, globals.frame * globals.diffuse_rays) + uvec4(0, 0, prd.cozw);
    const vec3 back = gl_WorldRayDirectionEXT * 0.001;

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

    vec3 world_space_pos = (gl_ObjectToWorldEXT * vec4(pos, 1.0));
    vec3 world_space_origin = world_space_pos - back;
    vec3 normal = normalize(v0.norm * barycentrics.x + v1.norm * barycentrics.y + v2.norm * barycentrics.z);
    vec4 tangh = v0.tangh * barycentrics.x + v1.tangh * barycentrics.y + v2.tangh * barycentrics.z;
    normal = local_normal(normal, tangh, uv);
    normal *= gl_HitKindEXT == gl_HitKindFrontFacingTriangleEXT ? 1 : -1;
    vec3 world_space_normal = normalize((gl_ObjectToWorldEXT * vec4(normal, 0.0)));

    vec3 radiance = vec3(0);

    if (dot(globals.dirlight.rad, vec3(1, 1, 1)) > 0.0001)
    {
        float attenuation = -dot(normalize(globals.dirlight.dir), world_space_normal);
        if (attenuation > 0.0)
        {
            float ray_contribution = attenuation / shadow_rays;

            unshadows = 0;
            for (uint i = 0; i < shadow_rays; ++i)
            {
                vec3 r = normalize(blue_rand_sphere(co + uvec4(0, 0, 0, i)) - globals.dirlight.dir);
                traceRayEXT(tlas, shadow_ray_flags, 0xff, 0, 0, 2, world_space_origin, 0, r, 1000.0, 1);
            }
            radiance += globals.dirlight.rad * (ray_contribution * unshadows);
        }
    }

    for (uint i = 0; i < globals.plights; ++i)
    {
        if (dot(plight[i].rad, vec3(1, 1, 1)) > 0.0001)
        {
            vec3 tolight = plight[i].pos - world_space_pos;
            float attenuation = dot(normalize(tolight), world_space_normal);
            if (attenuation > 0.0)
            {
                float ls = dot(tolight, tolight);
                float l = sqrt(ls);
                float ray_contribution = attenuation / shadow_rays / ls;

                unshadows = 0;
                for (int i = 0; i < shadow_rays; ++i)
                {
                    vec3 r = normalize(blue_rand_sphere(co + uvec4(0, 0, 0, i + shadow_rays)) + tolight);
                    traceRayEXT(tlas, shadow_ray_flags, 0xff, 0, 0, 2, world_space_origin, 0, r, l, 1);
                }
                radiance += plight[i].rad * (ray_contribution * unshadows);
            }
        }
    }

    radiance += query_diffuse_from_probes(world_space_origin, world_space_normal);
    radiance *= sample_albedo(uv).rgb;

    prd.result += radiance;
}
