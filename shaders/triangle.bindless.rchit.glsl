#version 460
#extension GL_NV_ray_tracing : require
#extension GL_EXT_nonuniform_qualifier : require

layout(set = 0, binding = 2) uniform UniformBuffer
{
    vec3 color;
} uniformBuffers[];

layout(location = 0) rayPayloadInNV vec3 hitValue;
hitAttributeNV vec3 attribs;

void main()
{
	// gl_InstanceCustomIndex = GeometryInstance::instanceId
    const vec3 color = uniformBuffers[nonuniformEXT(gl_InstanceCustomIndexNV)].color.xyz;
    hitValue = color;
}