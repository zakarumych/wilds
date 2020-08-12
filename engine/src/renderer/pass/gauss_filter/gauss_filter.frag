#version 460

layout(binding = 0, set = 0) uniform sampler2D normals_depth;
layout(binding = 1, set = 0) uniform sampler2D diffuse_input;

layout(location = 0) out vec4 output_color;

float kernel[4][4] = {
    { 1, 6, 15, 20 },
    { 6, 36, 90, 120 },
    { 15, 90, 225, 300 },
    { 20, 120, 300, 400 }
};

void main() {
    vec3 normal = texture(normals_depth, gl_FragCoord.xy).xyz;
    if (dot(normal, normal) < 0.9) {
        output_color = vec4(0, 0, 0, 1);
        return;
    }

    float sum = 0;
    vec3 diffuse = vec3(0, 0, 0);

    for (int y = -3; y <= 3; ++y) {
        for (int x = -3; x <= 3; ++x) {
            vec2 xy = vec2(x, y);
            if (dot(normal, texture(normals_depth, gl_FragCoord.xy + xy).xyz) > 0.8)
            {
                float w = 1 / (pow(length(xy), 0.2) + 1);
                sum += w;
                diffuse += w * texture(diffuse_input, gl_FragCoord.xy + xy).rgb;
            }
        }
    }
    output_color = vec4(diffuse / sum, 1);
}
