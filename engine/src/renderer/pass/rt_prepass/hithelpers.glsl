
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
    return normalize(vec3(raw.xy * instances[gl_InstanceID].normals_factor, raw.z));
}

vec3 local_normal(vec3 vertex_normal, vec4 tangh, vec2 uv) {
    // uint sampler_index = instances[gl_InstanceID].normals_sampler;
    // if (sampler_index > 0)
    // {
    //     vec3 raw = texture(normal[sampler_index-1], uv).xyz;
    //     vec3 sampled_normal = normalize(vec3(raw.xy * instances[gl_InstanceID].normals_factor, raw.z));

    //     vec3 bitang = cross(vertex_normal, tangh.xyz) * tangh.w;
    //     mat3 tang_space = mat3(bitang, tangh.xyz, vertex_normal);
    //     return tang_space * sampled_normal;
    // }
    // else
    {
        return vertex_normal;
    }
}