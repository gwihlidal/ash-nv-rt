#version 460
#extension GL_NV_ray_tracing : require

layout(set = 0, binding = 0) uniform accelerationStructureNV topLevelAS;
layout(set = 0, binding = 1, rgba8) uniform image2D image;

layout(location = 0) rayPayloadNV vec3 hitValue;

void main() 
{
    const vec2 pixelCenter = vec2(gl_LaunchIDNV.xy) + vec2(0.5);
    const vec2 inUV = pixelCenter/vec2(gl_LaunchSizeNV.xy);

    vec2 d = inUV * 2.0 - 1.0;
    float aspectRatio = float(gl_LaunchSizeNV.x) / float(gl_LaunchSizeNV.y);

    vec3 origin = vec3(0, 0, -2.0);
    vec3 direction = normalize(vec3(d.x * aspectRatio, -d.y, 1));
    uint rayFlags = gl_RayFlagsOpaqueNV;
    uint cullMask = 0xff;
    float tmin = 0.001;
    float tmax = 100.0;
    traceNV(topLevelAS, rayFlags, cullMask, 0 /*sbtRecordOffset*/, 0 /*sbtRecordStride*/, 0 /*missIndex*/, origin, tmin, direction, tmax, 0 /*payload*/);

    imageStore(image, ivec2(gl_LaunchIDNV.xy), vec4(hitValue, 0.0));
}
