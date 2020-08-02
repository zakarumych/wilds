#version 460

layout(binding = 0, set = 0) uniform sampler2D albedo;
layout(binding = 1, set = 0) uniform sampler2D normals_depth;
layout(binding = 2, set = 0) uniform sampler2D emissive_direct;
layout(binding = 3, set = 0) uniform sampler2D diffuse;

layout(location = 0) out vec4 output_color;

void main() {
    vec3 albedo = texture(albedo, gl_FragCoord.xy).rgb;
    vec4 emissive_direct = texture(emissive_direct, gl_FragCoord.xy);
    vec4 normals_depth = texture(normals_depth, gl_FragCoord.xy);
    vec3 diffuse = texture(diffuse, gl_FragCoord.xy).xyz;
    vec3 emissive = emissive_direct.rgb;
    float direct = emissive_direct.a;
    direct *= dot(normals_depth.xyz, vec3(0, 1, 0));
    vec3 combined = albedo * (direct * vec3(5.0, 3.0, 1.0) / 3 + diffuse) + emissive;
    output_color = vec4(combined / (vec3(1, 1, 1) + combined), 1);
}
