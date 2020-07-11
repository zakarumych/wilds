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
    prd.hit_value = barycentrics;

    // uvec3 indices = instance_triangle_indices(gl_InstanceID, gl_PrimitiveID);

    // Vertex v0 = instance_vertex(gl_InstanceID, indices.x);
    // Vertex v1 = instance_vertex(gl_InstanceID, indices.y);
    // Vertex v2 = instance_vertex(gl_InstanceID, indices.z);

    // vec3 normal = v0.norm * barycentrics.x + v1.norm * barycentrics.y + v2.norm * barycentrics.z;
    // vec3 pos = v0.pos * barycentrics.x + v1.pos * barycentrics.y + v2.pos * barycentrics.z;

    // // Transforming the normal to world space
    // normal = normalize((instance_transform(gl_InstanceID) * vec4(normal, 0.0)).xyz);

    // if (dot(normal, gl_WorldRayDirectionEXT) > 0)
    // {
    //     normal = -normal;
    // }

    // pos = (instance_transform(gl_InstanceID) * vec4(pos, 1.0)).xyz;

    // vec3 nx = normalize(cross(normal, normal.x == 0 ? vec3(1, 0, 0) : vec3(0, 1, 0)));
    // vec3 ny = normalize(cross(normal, nx));
    // mat3 nm = mat3(nx, ny, normal);

    // if (length(globals.dirlight.rad) > 0.0)
    // {
    //     vec3 tolight = -globals.dirlight.dir;
    //     if (dot(normal, tolight) > 0)
    //     {
    //         unshadows = 0;
    //         for (int i = 0; i < shadow_rays; ++i)
    //         {
    //             float x = blue_rand(i ^ int(prd.co.x));
    //             float y = blue_rand(i ^ int(prd.co.y));
    //             vec3 r = nm * vec3(x, y, 0.0);
    //             traceRayEXT(tlas, light_ray_flags, 0xff, 0, 0, 1, pos.xyz, 0.001, normalize(tolight + r), 1000.0, 0);
    //         }
    //         prd.hit_value += (globals.dirlight.rad * unshadows) / shadow_rays;
    //     }
    // }

    // if (prd.depth > 0)
    // {
    //     prd_reflect.hit_value = vec3(0, 0, 0);
    //     prd_reflect.depth = prd.depth - 1;
    //     prd_reflect.co = prd.co;

    //     vec2 xy = blue_rand_c(prd.co.xy);
    //     float z = blue_rand(prd.co.x) * 2 - 1;
    //     // vec3 dir = nm * normalize(vec3(xy, 1.0));
    //     vec3 dir = normalize(normal + blue_rand_unit_vector(prd.co));

    //     // vec3 dir = normalize(blue_rand_unit_vector(prd.co) + normal);
    //     traceRayEXT(tlas, 0, 0xff, 0, 0, 0, pos, 0.01, normalize(dir), 1000.0, 1);
    //     prd.hit_value += prd_reflect.hit_value / 2;
    // }

    // prd.hit_value *= vec3(0.5, 0.2, 0.1);
}
