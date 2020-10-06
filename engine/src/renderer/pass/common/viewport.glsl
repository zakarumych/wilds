

void traceViewportPixelRay()
{
    const uint shadow_ray_flags = gl_RayFlagsOpaqueEXT | gl_RayFlagsTerminateOnFirstHitEXT | gl_RayFlagsSkipClosestHitShaderEXT;
    const uvec3 co = uvec3(gl_LaunchIDEXT.xy, globals.frame * globals.diffuse_rays);
    const vec2 pixelCenter = vec2(gl_LaunchIDEXT.xy);
    const vec2 inUV = pixelCenter/vec2(gl_LaunchSizeEXT.xy);
    vec2 d = inUV * 2.0 - 1.0;
    vec4 proj  = globals.cam.iproj * vec4(d.x, -d.y, -1, 1);
    vec4 target = globals.cam.view * vec4(normalize(proj.xyz), 1.0);

    vec4 origin = globals.cam.view[3];

    traceRayEXT(tlas, 0, 0xff, 0, 0, 0, origin.xyz, 0.0, normalize((target - origin).xyz), 1000.0, 0);
}
