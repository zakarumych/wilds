

void cube_blend(vec3 dir, out float faces[6]) {
    faces[0] = float(dir.x > -.2) * (abs(dir.x) + .2);
    faces[1] = float(dir.x < +.2) * (abs(dir.x) + .2);
    faces[2] = float(dir.y > -.2) * (abs(dir.y) + .2);
    faces[3] = float(dir.y < +.2) * (abs(dir.y) + .2);
    faces[4] = float(dir.z > -.2) * (abs(dir.z) + .2);
    faces[5] = float(dir.z < +.2) * (abs(dir.z) + .2);
}


void cube_blend_strict(vec3 dir, out float faces[6]) {
    faces[0] = float(dir.x > 0) * (dir.x * dir.x);
    faces[1] = float(dir.x < 0) * (dir.x * dir.x);
    faces[2] = float(dir.y > 0) * (dir.y * dir.y);
    faces[3] = float(dir.y < 0) * (dir.y * dir.y);
    faces[4] = float(dir.z > 0) * (dir.z * dir.z);
    faces[5] = float(dir.z < 0) * (dir.z * dir.z);
}

float get_cube_probe(vec3 dir, uvec3 probe) {
    float faces[6];
    cube_blend(dir, faces);

    ivec2 uv = ivec2(probe.x + globals.probes_extent.x * probe.z, probe.y * 6);
    float result = 0.;

    for (int i = 0; i < 6; ++i) {
        if (faces[i] > 0)
            result += imageLoad(probes, uv + ivec2(0, i)).r * faces[i];
    }

    return result;
}

vec2 get_cube_probe_2(vec3 dir, uvec3 probe) {
    float faces[6];
    cube_blend(dir, faces);

    ivec2 uv = ivec2(probe.x + globals.probes_extent.x * probe.z, probe.y * 6);
    vec2 result = vec2(0,0);

    for (int i = 0; i < 6; ++i) {
        if (faces[i] > 0)
            result += imageLoad(probes, uv + ivec2(0, i)).rg * faces[i];
    }

    return result;
}

vec3 get_cube_probe_3(vec3 dir, uvec3 probe) {
    float faces[6];
    cube_blend(dir, faces);

    ivec2 uv = ivec2(probe.x + globals.probes_extent.x * probe.z, probe.y * 6);
    vec3 result = vec3(0, 0, 0);
    float total = 0.;

    for (int i = 0; i < 6; ++i) {
        if (faces[i] > 0) {
            result += imageLoad(probes, uv + ivec2(0, i)).rgb * faces[i];
            total += faces[i];
        }
    }

    return result / total;
}

vec3 probe_cell_size() {
    return globals.probes_dimensions / vec3(globals.probes_extent - uvec3(1, 1, 1));
}
