struct Payload
{
    float3 hitValue;
};

struct Attribute
{
    float2 bary;
};

struct UniformBuffer
{
	float3 color;
};

ConstantBuffer<UniformBuffer> g_uniformBuffers[] : register(b2, space0);

[shader("closesthit")]
void main(inout Payload payload : SV_RayPayload, in Attribute attribs : SV_IntersectionAttributes)
{
	// InstanceID = GeometryInstance::instanceId
	const float3 color = g_uniformBuffers[NonUniformResourceIndex(InstanceID())].color.xyz;
    payload.hitValue = color;
}
