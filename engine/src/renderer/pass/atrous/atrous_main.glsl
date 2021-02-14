

#define EULERS 2.71828182845904523536028747135266250

float kern[] = {
    0.0625,
    0.25,
    0.375,
    0.25,
    0.0625
};

float albedo_sigma = 0.1;
float normal_sigma = 0.1;
float depth_sigma = 0.1;

void main() {
    vec4 normal_depth_p = texture(normals_depth, gl_FragCoord.xy);
    vec3 normal_p = normal_depth_p.xyz;
    float depth_p = normal_depth_p.w;
    vec3 albedo_p = texture(unfiltered, gl_FragCoord.xy).rgb;

    float sum = 0;
    vec3 filtered = vec3(0, 0, 0);

    for (int y = -h; y <= h; y += l) {
        for (int x = -w; x <= w; x += l) {
            vec2 xy = vec2(x, y);
            vec4 normal_depth_q = texture(normals_depth, gl_FragCoord.xy + xy);
            vec3 normal_q = normal_depth_q.xyz;
            float depth_q = normal_depth_q.w;
            vec3 albedo_q = texture(unfiltered, gl_FragCoord.xy + xy).rgb;

            float wh = kern[(x + y) / l];
            float wa = pow(EULERS, -(length(albedo_p - albedo_q) / albedo_sigma));
            float wn = pow(EULERS, -(length(normal_p - normal_q) / normal_sigma));
            float wd = pow(EULERS, -(length(depth_p - depth_q) / depth_sigma));

            float W = wh * wa * wn * wd;

            sum += W;
            filtered += W * albedo_q;
       }
    }

    output_image = vec4(filtered / sum, 1.0);
}
