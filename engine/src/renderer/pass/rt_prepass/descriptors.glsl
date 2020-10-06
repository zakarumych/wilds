#extension GL_EXT_scalar_block_layout : enable
#extension GL_ARB_gpu_shader_int64 : enable

struct RecursiveRay {
    vec3 origin;
    vec3 direction;
};

struct PrimaryHitPayload {
    vec3 normal;
    float depth;
    vec4 albedo;
    vec3 emissive;
    vec3 direct;
    vec3 diffuse;
};

struct DiffuseHitPayload {
    vec3 radiation;
    uint ray_index;
};

struct Vertex {
    vec3 pos;
    vec3 norm;
    vec4 tangh;
    vec2 uv;
};

struct Instance {
    mat4 transform;
    uint mesh;
    uint albedo_sampler;
    vec4 albedo_factor;
    uint normals_sampler;
    float normals_factor;
    uint anim;
};

struct Camera {
    mat4 view;
    mat4 proj;
    mat4 iview;
    mat4 iproj;
};

struct DirLight {
    vec3 dir;
    float pad0;
    vec3 rad;
    float pad1;
};

struct PointLight {
    vec3 pos;
    vec3 rad;
};

layout(binding = 0, set = 0) uniform accelerationStructureEXT tlas;
layout(binding = 1, set = 0) buffer BlueNoise { vec4 blue_noise[256*256*128]; };
layout(binding = 2, set = 0, scalar) buffer Indices { uint i[]; } indices[];
layout(binding = 3, set = 0, scalar) buffer Vertices { Vertex v[]; } vertices[];
layout(binding = 4, set = 0) uniform sampler2D albedo[];
layout(binding = 5, set = 0) uniform sampler2D normal[];

layout(binding = 0, set = 1, std140) uniform Globals {
    Camera cam;
    DirLight dirlight;
    vec3 skylight;
    uint plights;
    uint frame;
    uint shadow_rays;
    uint diffuse_rays;
} globals;

layout(binding = 1, set = 1, scalar) buffer Scene { Instance instances[]; };
layout(binding = 2, set = 1, std140) buffer PointLights { PointLight plight[]; };
layout(binding = 3, set = 1, scalar) buffer AnimVertices { Vertex v[]; } anim_vertices[];
