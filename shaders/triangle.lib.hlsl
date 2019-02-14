struct Payload
{
    float3 hitValue;
};

struct Attribute
{
    float2 bary;
};

RaytracingAccelerationStructure g_topLevel : register(t0, space0);
RWTexture2D<float4> g_output : register(u1, space0);

[shader("raygeneration")]
void rgen_main()
{
    uint2 launchIndex = DispatchRaysIndex().xy;
    float2 dims = DispatchRaysDimensions().xy;

    float2 pixelCenter = launchIndex + 0.5;
    float2 uv = pixelCenter / dims.xy;

    float2 d = uv * 2.0 - 1.0;
    float aspectRatio = float(dims.x) / float(dims.y);

    RayDesc ray;
    ray.Origin = float3(0.0, 0.0, -2.0);
    ray.Direction = normalize(float3(d.x * aspectRatio, -d.y, 1.0));
    ray.TMin = 0.001;
    ray.TMax = 1000.0;

    Payload payload;
    payload.hitValue = float3(0.0, 0.0, 0.0);

    TraceRay(g_topLevel, RAY_FLAG_FORCE_OPAQUE, 0xff, 0, 0, 0, ray, payload);

    g_output[launchIndex] = float4(payload.hitValue, 1.0f);
}

[shader("miss")]
void rmiss_main(inout Payload payload : SV_RayPayload)
{
    payload.hitValue = float3(0.0, 0.1, 0.3);
}

[shader("closesthit")]
void rchit_main(inout Payload payload : SV_RayPayload, in Attribute attribs : SV_IntersectionAttributes)
{
    const float3 barycentrics = float3(1.0 - attribs.bary.x - attribs.bary.y, attribs.bary.x, attribs.bary.y);
    payload.hitValue = barycentrics;
}
