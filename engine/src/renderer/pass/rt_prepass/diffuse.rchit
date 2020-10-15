#version 460
#extension GL_EXT_ray_tracing : require
#extension GL_GOOGLE_include_directive : enable
#extension GL_EXT_scalar_block_layout : enable

#include "descriptors.glsl"
#include "../common/rayhit.glsl"
#include "../common/rand.glsl"

layout(location = 0) rayPayloadInEXT DiffuseHitPayload prd;
layout(location = 1) rayPayloadEXT uint unshadows;

hitAttributeEXT vec2 attribs;

void main() {
    const vec3 back = gl_WorldRayDirectionEXT * 0.001;
    const uint shadow_rays = 1;
    const uint diffuse_rays = 1;
    const uvec3 co = uvec3(gl_LaunchIDEXT.xy, globals.frame + prd.ray_index);

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
    normal = local_normal(normal, tangh, uv);

    vec3 world_space_normal = normalize((gl_ObjectToWorldEXT * vec4(normal, 0.0)));
    world_space_normal *= -sign(dot(world_space_normal, gl_WorldRayDirectionEXT));

    vec3 worls_space_pos = (gl_ObjectToWorldEXT * vec4(pos, 1.0));

    vec3 radiation = vec3(0.0, 0.0, 0.0);

    if (dot(globals.dirlight.rad, vec3(1, 1, 1)) > 0.0001)
    {
        float attenuation = -dot(normalize(globals.dirlight.dir), world_space_normal);
        if (attenuation > 0.0)
        {
            float ray_contribution = attenuation / shadow_rays;

            unshadows = 0;
            for (uint i = 0; i < shadow_rays; ++i)
            {
                vec3 r = normalize(rand_sphere(blue_rand(uvec4(co, i))) - globals.dirlight.dir);
                traceRayEXT(tlas, shadow_ray_flags, 0xff, 0, 0, 2, worls_space_pos - back, 0, r, 1000.0, 1);
            }
            radiation += globals.dirlight.rad * (ray_contribution * unshadows);
        }
    }

    // for (uint i = 0; i < globals.plights; ++i)
    // {
    //     if (dot(plight[i].rad, vec3(1, 1, 1)) > 0.0001)
    //     {
    //         vec3 tolight = plight[i].pos - worls_space_pos;
    //         float attenuation = dot(normalize(tolight), world_space_normal);
    //         if (attenuation > 0.0)
    //         {
    //             float ls = dot(tolight, tolight);
    //             float l = sqrt(ls);
    //             float ray_contribution = attenuation / shadow_rays / ls;

    //             unshadows = 0;
    //             for (int i = 0; i < shadow_rays; ++i)
    //             {
    //                 vec3 r = normalize(rand_sphere(blue_rand(uvec4(co, i + shadow_rays))) + tolight);
    //                 traceRayEXT(tlas, shadow_ray_flags, 0xff, 0, 0, 2, worls_space_pos - back, 0, r, l, 1);
    //             }
    //             radiation += plight[i].rad * (ray_contribution * unshadows);
    //         }
    //     }
    // }

    vec3 albedo = sample_albedo(uv).xyz;
    prd.radiation += radiation * albedo;
}
