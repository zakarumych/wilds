#version 460

layout(binding = 0, set = 0) uniform sampler2D albedo;
layout(binding = 1, set = 0) uniform sampler2D normals_depth;
layout(binding = 2, set = 0) uniform sampler2D emissive;
layout(binding = 3, set = 0) uniform sampler2D direct;
layout(binding = 4, set = 0) uniform sampler2D diffuse;

layout(location = 0) out vec4 output_color;

layout(push_constant) uniform push_constants { uvec2 screen_size; };

// vec4 draw_normal(vec3 n) {
//     return vec4(n / 2.0 + vec3(0.5, 0.5, 0.5), 1.0);
// }

// vec4 draw_depth(float d) {
//     d = d / (1 + d);
//     return vec4(d, d, d, 1.0);
// }

void main() {
    vec3 albedo = texture(albedo, gl_FragCoord.xy / screen_size).rgb;
    vec3 emissive = texture(emissive, gl_FragCoord.xy / screen_size).rgb;
    vec3 direct = texture(direct, gl_FragCoord.xy / screen_size).rgb;
    vec4 normals_depth = texture(normals_depth, gl_FragCoord.xy / screen_size);
    vec3 diffuse = texture(diffuse, gl_FragCoord.xy / screen_size).xyz;
    // direct *= dot(normals_depth.xyz, vec3(0, 1, 0));
    vec3 combined = albedo * (direct + diffuse) + emissive;
    output_color = vec4(combined / (vec3(1, 1, 1) + combined), 1);
}
