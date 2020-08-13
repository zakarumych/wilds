#version 460
#extension GL_EXT_ray_tracing : require
#extension GL_GOOGLE_include_directive : enable
#extension GL_EXT_scalar_block_layout : enable
#include "raycommon.glsl"

#include "rayhelpers.glsl"


uvec3 instance_triangle_indices() {
    uint mesh = instances[gl_InstanceID].mesh;
    return uvec3(indices[mesh].i[3 * gl_PrimitiveID + 0],
                 indices[mesh].i[3 * gl_PrimitiveID + 1],
                 indices[mesh].i[3 * gl_PrimitiveID + 2]);
}

Vertex instance_vertex(uint index) {
    uint mesh = instances[gl_InstanceID].mesh;
    return vertices[mesh].v[index];
}

mat4 instance_transform() {
    return instances[gl_InstanceID].transform;
}

vec4 sample_albedo(vec2 uv) {
    uint sampler_index = instances[gl_InstanceID].albedo_sampler;
    vec4 raw = vec4(1, 1, 1, 1);
    if (sampler_index > 0)
    {
        raw = texture(albedo[sampler_index-1], uv);
    }
    return raw * instances[gl_InstanceID].albedo_factor;
}

vec3 sample_normal(vec2 uv) {
    uint sampler_index = instances[gl_InstanceID].normals_sampler;
    vec3 raw = vec3(0, 0, 1);
    if (sampler_index > 0)
    {
        raw = texture(normal[sampler_index-1], uv).xyz;
    }
    return vec3(raw.xy * instances[gl_InstanceID].normals_factor, raw.z);
}

float components_sum(vec3 v) {
    return v.x + v.y + v.z;
}

layout(location = 0) rayPayloadInEXT PrimaryHitPayload prd;
layout(location = 1) rayPayloadEXT uint unshadows;
layout(location = 2) rayPayloadEXT DiffuseHitPayload dprd;

hitAttributeEXT vec2 attribs;

void main()
{
    const uint shadow_rays = 2;
    const uint shadow_ray_flags = gl_RayFlagsOpaqueEXT | gl_RayFlagsTerminateOnFirstHitEXT | gl_RayFlagsSkipClosestHitShaderEXT;
    const vec3 barycentrics = vec3(1.0f - attribs.x - attribs.y, attribs.x, attribs.y);
    uvec3 indices = instance_triangle_indices();

    Vertex v0 = instance_vertex(indices.x);
    Vertex v1 = instance_vertex(indices.y);
    Vertex v2 = instance_vertex(indices.z);

    vec3 pos = v0.pos * barycentrics.x + v1.pos * barycentrics.y + v2.pos * barycentrics.z;
    vec3 normal = normalize(v0.norm * barycentrics.x + v1.norm * barycentrics.y + v2.norm * barycentrics.z);
    vec4 tangh = normalize(v0.tangh * barycentrics.x + v1.tangh * barycentrics.y + v2.tangh * barycentrics.z);
    vec3 tang = tangh.xyz;
    vec2 uv = v0.uv * barycentrics.x + v1.uv * barycentrics.y + v2.uv * barycentrics.z;
    vec3 bitang = normalize(cross(normal, tang)) * tangh.w;

    mat3 tang_space = mat3(bitang, tang, normal);
    vec3 sampled_normal = sample_normal(uv);
    vec3 world_space_normal = normalize((instance_transform() * vec4(normal, 0.0)).xyz);
    vec3 worls_space_pos = (instance_transform() * vec4(pos, 1.0)).xyz;

    prd.albedo = sample_albedo(uv);
    prd.normal = world_space_normal;
    prd.depth = gl_HitTEXT;

    float light_distance = length(globals.dirlight.dir);
    vec3 light_direction = normalize(-globals.dirlight.dir);

    float dirlight_attenuation = dot(light_direction, world_space_normal);
    if (dirlight_attenuation > 0.0 && components_sum(globals.dirlight.rad) > 0.0)
    {
        float ray_contribution = dirlight_attenuation / shadow_rays;

        // vec3 lx = cross(vec3(1, 0, 0), light_direction);
        // vec3 ly = cross(vec3(0, 1, 0), light_direction);
        // vec3 lz = cross(vec3(0, 0, 1), light_direction);
        // vec3 lo = normalize(max(lx, max(ly, lz)));
        // vec3 lt = normalize(cross(lo, light_direction));

        // mat3 light_space = mat3(lt, lo, light_direction);

        unshadows = 0;
        for (int i = 0; i < shadow_rays; ++i)
        {
            // vec3 r = tang_space * blue_rand_cone(prd.co + uvec3(i * 137), .8);
            // vec3 r = blue_rand_cone_dir(prd.co + uvec3(i * 137), light_distance / (1 + light_distance), light_direction);

            vec3 r = normalize(blue_rand_sphere(prd.co + uvec3(0, i, i)) - globals.dirlight.dir);
            traceRayEXT(tlas, shadow_ray_flags, 0xff, 0, 0, 2, worls_space_pos, 0.01, r, 1000.0, 1);
        }
        prd.direct = globals.dirlight.rad * (ray_contribution * unshadows);
    }

    for (int i = 0; i < 2; ++i)
    {
        // vec3 dir = normalize(tang_space * blue_rand_hemisphere_cosine(prd.co + uvec3(i, i * 131, i * 65537)));
        vec3 dir = blue_rand_hemisphere_cosine_dir(prd.co + uvec3(i, 0, i), normal);
        dprd.radiation = vec3(0, 0, 0);
        traceRayEXT(tlas, 00, 0xff, 1, 0, 1, worls_space_pos, 0.001, dir, 1000.0, 2);
        prd.diffuse += dprd.radiation / 2;
    }
}
