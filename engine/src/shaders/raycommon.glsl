#extension GL_EXT_scalar_block_layout : enable
#extension GL_EXT_shader_16bit_storage : enable
#extension GL_ARB_gpu_shader_int64 : enable

#define M_PI 3.1415926535897932384626433832795
#define M_FI 1.61803398874989484820458683436563811772030917980576286213544862270526046281890244970720720418939113748475

struct RecursiveRay {
  vec3 origin;
  vec3 direction;
};

struct HitPayload {
  vec3 hit_value;
  vec2 co;
  int depth;
};

struct LightHitPayload {
  bool shadowed;
};

struct Vertex {
  vec3 pos;
  vec3 norm;
};

struct SceneInstance {
  mat4 transform;
  uint mesh;
};

struct Camera {
  mat4 view;
  mat4 proj;
  mat4 iview;
  mat4 iproj;
};

struct DirLight {
  vec3 dir;
  vec3 rad;
};

layout(binding = 0, set = 0) uniform accelerationStructureEXT tlas;
layout(binding = 1, set = 0) uniform Globals {
  Camera cam;
  DirLight dirlight;
  float seconds;
  uint frame;
} globals;

layout(binding = 2, set = 0, scalar) buffer Scene { SceneInstance i[]; } scene;
layout(binding = 3, set = 0, scalar) buffer Vertices { Vertex v[]; } vertices[];
layout(binding = 4, set = 0, scalar) buffer Indices { uint i[]; } indices[];

