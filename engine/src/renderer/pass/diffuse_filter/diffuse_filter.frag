#version 460

layout(binding = 0, set = 0) uniform sampler2D diffuse_input;

layout(location = 0) out vec4 output_color;

float kernel[4][4] = {
    { 1, 6, 15, 20 },
    { 6, 36, 90, 120 },
    { 15, 90, 225, 300 },
    { 20, 120, 300, 400 }
};

void main() {
    float sum = 0;
    vec3 diffuse = vec3(0, 0, 0);
    for (int y = -3; y <= 3; ++y) {
        for (int x = -3; x <= 3; ++x) {
            vec2 xy = vec2(x, y);
            float w = 1 / (pow(length(xy), 0.2) + 1);
            // float w = kernel[abs(x)][abs(y)];
            sum += w;
            diffuse += w * texture(diffuse_input, gl_FragCoord.xy + xy).rgb;
        }
    }
    output_color = vec4(diffuse / sum, 1);
}
