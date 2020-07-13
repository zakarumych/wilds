#version 460
#extension GL_EXT_ray_tracing : require
#extension GL_GOOGLE_include_directive : enable
#extension GL_EXT_scalar_block_layout : enable
#include "raycommon.glsl"

#include "rayhelpers.glsl"

layout(location = 0) rayPayloadEXT uint unshadows;
layout(location = 1) rayPayloadEXT HitPayload prd_reflect;

layout(location = 0) rayPayloadInEXT HitPayload prd;

hitAttributeEXT vec2 attribs;

const vec3 INDEX_COLORS[6] = { vec3(1.0, 0.0, 0.0), vec3(0.0, 1.0, 0.0), vec3(0.0, 0.0, 1.0), vec3(1.0, 1.0, 0.0), vec3(1.0, 0.0, 1.0), vec3(0.0, 1.0, 1.0) };

void main()
{
    const uint shadow_rays = 8;

    const vec3 color = vec3(0.3, 0.7, 0.5);
    const uint light_ray_flags = gl_RayFlagsOpaqueEXT | gl_RayFlagsTerminateOnFirstHitEXT | gl_RayFlagsSkipClosestHitShaderEXT;

    const vec3 barycentrics = vec3(1.0f - attribs.x - attribs.y, attribs.x, attribs.y);

    uvec3 indices = instance_triangle_indices(gl_InstanceID, gl_PrimitiveID);

    Vertex v0 = instance_vertex(gl_InstanceID, indices.x);
    Vertex v1 = instance_vertex(gl_InstanceID, indices.y);
    Vertex v2 = instance_vertex(gl_InstanceID, indices.z);

    vec3 pos = v0.pos * barycentrics.x + v1.pos * barycentrics.y + v2.pos * barycentrics.z;
    vec3 normal = normalize(v0.norm * barycentrics.x + v1.norm * barycentrics.y + v2.norm * barycentrics.z);
    vec4 tangh = normalize(v0.tangh * barycentrics.x + v1.tangh * barycentrics.y + v2.tangh * barycentrics.z);
    vec3 tang = tangh.xyz;
    vec2 uv = v0.uv * barycentrics.x + v1.uv * barycentrics.y + v2.uv * barycentrics.z;
    vec3 bitang = normalize(cross(normal, tang)) * tangh.w;

    mat3 tang_space = mat3(bitang, tang, normal);
    vec3 sampled_normal = sample_normal(gl_InstanceID, uv);
    vec3 world_space_normal = normalize((instance_transform(gl_InstanceID) * vec4(normal, 0.0)).xyz);

    if (dot(world_space_normal, gl_WorldRayDirectionEXT) > 0)
    {
        world_space_normal = -world_space_normal;
    }

    vec3 worls_space_pos = (instance_transform(gl_InstanceID) * vec4(pos, 1.0)).xyz;

    if (length(globals.dirlight.rad) > 0.0)
    {
        vec3 tolight = -globals.dirlight.dir;
        float affect = 1;//dot(world_space_normal, tolight);
        if (affect > 0)
        {
            unshadows = 0;
            for (int i = 0; i < shadow_rays; ++i)
            {
                vec3 r = tang_space * vec3(blue_rand_circle(uvec3(prd.co.xy, prd.co.z + i)), 0.0);
                traceRayEXT(tlas, light_ray_flags, 0xff, 0, 0, 1, worls_space_pos.xyz, 0.001, normalize(tolight + r), 1000.0, 0);
            }
            prd.hit_value += (globals.dirlight.rad * affect * unshadows) / shadow_rays;
        }
    }

    if (prd.depth > 0)
    {
        prd_reflect.hit_value = vec3(0, 0, 0);
        prd_reflect.depth = prd.depth - 1;
        prd_reflect.co = prd.co;

        vec3 dir = normalize(world_space_normal + blue_rand_unit_vector(prd.co));
        traceRayEXT(tlas, 0, 0xff, 0, 0, 0, worls_space_pos, 0.01, dir, 1000.0, 1);
        prd.hit_value += prd_reflect.hit_value / 2;
    }

    vec4 albedo = sample_albedo(gl_InstanceID, uv);

    prd.hit_value *= albedo.rgb;
    // prd.hit_value = world_space_normal / 2 + vec3(0.5, 0.5, 0.5);
}
