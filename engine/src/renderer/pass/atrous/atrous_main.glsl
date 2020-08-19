

void main() {
    vec4 normal_depth = texture(normals_depth, gl_FragCoord.xy);
    vec3 normal = normal_depth.xyz;
    if (dot(normal, normal) < 0.9) {
        output_image = vec4(0, 0, 0, 1);
        return;
    }
    float depth = normal_depth.w;

    // float kern[r+1];
    // kern[r] = 1.0 / exp2(float(r) * 2.0);

    // float sum = kern[r];
    // float n = 0.0;
    // for (int i = r - 1; i > 0; --i, ++n) {
    //     kern[i] = kern[i+1] * ((float(r) * 2.0 - n) / (n + 1.0));
    //     sum += kern[i];
    // }

    // kern[0] = 1.0 - 2.0 * sum;

    float sum = 0;
    vec3 filtered = vec3(0, 0, 0);

    for (int y = -h; y <= h; y += l) {
        for (int x = -w; x <= w; x += l) {
            vec2 xy = vec2(x, y);
            vec4 normal_depth = texture(normals_depth, gl_FragCoord.xy + xy);
            float depth_factor = max(0, 1 - abs(normal_depth.w - depth));
            if (depth_factor > 0.8) {
                float normal_factor = dot(normal, normal_depth.xyz);
                if (normal_factor > 0.7) {
                    float f = 1 / (1 + length(xy));
                    sum += f;
                    filtered += f * texture(unfiltered, gl_FragCoord.xy + xy).rgb;
                }
            }
        }
    }

    output_image = vec4(filtered / sum, 1);
}
