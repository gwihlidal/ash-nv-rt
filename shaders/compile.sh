echo "** Compiling triangle.bindless.rchit.glsl"
docker run --entrypoint "/app/vulkan/glslangValidator" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -V -o compiled/triangle.glsl_bindless_rchit.spv triangle.bindless.rchit.glsl

echo "** Compiling triangle.bindless.rchit.hlsl"
docker run --entrypoint "/app/dxc/bin/dxc" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -T lib_6_3 triangle.bindless.rchit.hlsl -Fo compiled/triangle.hlsl_bindless_rchit.dxil
docker run --entrypoint "/app/dxc/bin/dxc" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -T lib_6_3 -spirv triangle.bindless.rchit.hlsl -Fo compiled/triangle.hlsl_bindless_rchit.spv

echo "** Compiling triangle.lib.hlsl"
docker run --entrypoint "/app/dxc/bin/dxc" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -T lib_6_3 triangle.lib.hlsl -Fo compiled/triangle.hlsl_lib.dxil
docker run --entrypoint "/app/dxc/bin/dxc" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -T lib_6_3 -spirv triangle.lib.hlsl -Fo compiled/triangle.hlsl_lib.spv

echo "** Compiling triangle.rchit.glsl"
docker run --entrypoint "/app/vulkan/glslangValidator" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -V -o compiled/triangle.glsl_rchit.spv triangle.rchit.glsl

echo "** Compiling triangle.rchit.hlsl"
docker run --entrypoint "/app/dxc/bin/dxc" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -T lib_6_3 triangle.rchit.hlsl -Fo compiled/triangle.hlsl_rchit.dxil
docker run --entrypoint "/app/dxc/bin/dxc" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -T lib_6_3 -spirv triangle.rchit.hlsl -Fo compiled/triangle.hlsl_rchit.spv

echo "** Compiling triangle.rgen.glsl"
docker run --entrypoint "/app/vulkan/glslangValidator" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -V -o compiled/triangle.glsl_rgen.spv triangle.rgen.glsl

echo "** Compiling triangle.rgen.hlsl"
docker run --entrypoint "/app/dxc/bin/dxc" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -T lib_6_3 triangle.rgen.hlsl -Fo compiled/triangle.hlsl_rgen.dxil
docker run --entrypoint "/app/dxc/bin/dxc" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -T lib_6_3 -spirv triangle.rgen.hlsl -Fo compiled/triangle.hlsl_rgen.spv

echo "** Compiling triangle.rmiss.glsl"
docker run --entrypoint "/app/vulkan/glslangValidator" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -V -o compiled/triangle.glsl_rmiss.spv triangle.rmiss.glsl

echo "** Compiling triangle.rmiss.hlsl"
docker run --entrypoint "/app/dxc/bin/dxc" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -T lib_6_3 triangle.rmiss.hlsl -Fo compiled/triangle.hlsl_rmiss.dxil
docker run --entrypoint "/app/dxc/bin/dxc" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -T lib_6_3 -spirv triangle.rmiss.hlsl -Fo compiled/triangle.hlsl_rmiss.spv

echo "** Disassembling SPIR-V"
docker run --entrypoint "/app/vulkan/spirv-dis" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -o compiled/triangle.glsl_bindless_rchit.txt compiled/triangle.glsl_bindless_rchit.spv
docker run --entrypoint "/app/vulkan/spirv-dis" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -o compiled/triangle.glsl_rchit.txt compiled/triangle.glsl_rchit.spv
docker run --entrypoint "/app/vulkan/spirv-dis" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -o compiled/triangle.glsl_rgen.txt compiled/triangle.glsl_rgen.spv
docker run --entrypoint "/app/vulkan/spirv-dis" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -o compiled/triangle.glsl_rmiss.txt compiled/triangle.glsl_rmiss.spv
docker run --entrypoint "/app/vulkan/spirv-dis" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -o compiled/triangle.hlsl_bindless_rchit.txt compiled/triangle.hlsl_bindless_rchit.spv
docker run --entrypoint "/app/vulkan/spirv-dis" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -o compiled/triangle.hlsl_lib.txt compiled/triangle.hlsl_lib.spv
docker run --entrypoint "/app/vulkan/spirv-dis" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -o compiled/triangle.hlsl_rchit.txt compiled/triangle.hlsl_rchit.spv
docker run --entrypoint "/app/vulkan/spirv-dis" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -o compiled/triangle.hlsl_rgen.txt compiled/triangle.hlsl_rgen.spv
docker run --entrypoint "/app/vulkan/spirv-dis" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -o compiled/triangle.hlsl_rmiss.txt compiled/triangle.hlsl_rmiss.spv

echo "** DONE **"