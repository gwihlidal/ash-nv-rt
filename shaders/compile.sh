docker run --entrypoint "/app/vulkan/glslangValidator" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -V -o compiled/triangle.glsl_bindless_rchit.spv triangle.bindless.rchit.glsl

docker run --entrypoint "/app/dxc/bin/dxc" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -T lib_6_3 triangle.bindless.rchit.hlsl -Fo compiled/triangle.hlsl_bindless_rchit.dxil
docker run --entrypoint "/app/dxc/bin/dxc" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -T lib_6_3 -spirv triangle.bindless.rchit.hlsl -Fo compiled/triangle.hlsl_bindless_rchit.spv

docker run --entrypoint "/app/dxc/bin/dxc" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -T lib_6_3 triangle.lib.hlsl -Fo compiled/triangle.hlsl_lib.dxil
docker run --entrypoint "/app/dxc/bin/dxc" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -T lib_6_3 -spirv triangle.lib.hlsl -Fo compiled/triangle.hlsl_lib.spv

docker run --entrypoint "/app/vulkan/glslangValidator" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -V -o compiled/triangle.glsl_rchit.spv triangle.rchit.glsl

docker run --entrypoint "/app/dxc/bin/dxc" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -T lib_6_3 triangle.rchit.hlsl -Fo compiled/triangle.hlsl_rchit.dxil
docker run --entrypoint "/app/dxc/bin/dxc" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -T lib_6_3 -spirv triangle.rchit.hlsl -Fo compiled/triangle.hlsl_rchit.spv

docker run --entrypoint "/app/vulkan/glslangValidator" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -V -o compiled/triangle.glsl_rgen.spv triangle.rgen.glsl

docker run --entrypoint "/app/dxc/bin/dxc" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -T lib_6_3 triangle.rgen.hlsl -Fo compiled/triangle.hlsl_rgen.dxil
docker run --entrypoint "/app/dxc/bin/dxc" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -T lib_6_3 -spirv triangle.rgen.hlsl -Fo compiled/triangle.hlsl_rgen.spv

docker run --entrypoint "/app/vulkan/glslangValidator" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -V -o compiled/triangle.glsl_rmiss.spv triangle.rmiss.glsl

docker run --entrypoint "/app/dxc/bin/dxc" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -T lib_6_3 triangle.rmiss.hlsl -Fo compiled/triangle.hlsl_rmiss.dxil
docker run --entrypoint "/app/dxc/bin/dxc" --rm -v $(pwd):$(pwd) -w $(pwd) gwihlidal/docker-shader -T lib_6_3 -spirv triangle.rmiss.hlsl -Fo compiled/triangle.hlsl_rmiss.spv