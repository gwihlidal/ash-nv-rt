REM "** Compiling triangle.bindless.rchit.hlsl"
dxc.exe -T lib_6_3 triangle.bindless.rchit.hlsl -Fo compiled/triangle.hlsl_bindless_rchit.dxil
dxc.exe -T lib_6_3 -spirv triangle.bindless.rchit.hlsl -Fo compiled/triangle.hlsl_bindless_rchit.spv

REM "** Compiling triangle.lib.hlsl"
dxc.exe -T lib_6_3 triangle.lib.hlsl -Fo compiled/triangle.hlsl_lib.dxil
dxc.exe -T lib_6_3 -spirv triangle.lib.hlsl -Fo compiled/triangle.hlsl_lib.spv

REM "** Compiling triangle.rchit.hlsl"
dxc.exe -T lib_6_3 triangle.rchit.hlsl -Fo compiled/triangle.hlsl_rchit.dxil
dxc.exe -T lib_6_3 -spirv triangle.rchit.hlsl -Fo compiled/triangle.hlsl_rchit.spv

echo "** Compiling triangle.rgen.hlsl"
dxc.exe -T lib_6_3 triangle.rgen.hlsl -Fo compiled/triangle.hlsl_rgen.dxil
dxc.exe -T lib_6_3 -spirv triangle.rgen.hlsl -Fo compiled/triangle.hlsl_rgen.spv

REM "** Compiling triangle.rmiss.hlsl"
dxc.exe -T lib_6_3 triangle.rmiss.hlsl -Fo compiled/triangle.hlsl_rmiss.dxil
dxc.exe -T lib_6_3 -spirv triangle.rmiss.hlsl -Fo compiled/triangle.hlsl_rmiss.spv

REM "** Disassembling SPIR-V"
spirv-dis.exe -o compiled/triangle.hlsl_bindless_rchit.txt compiled/triangle.hlsl_bindless_rchit.spv
spirv-dis.exe -o compiled/triangle.hlsl_lib.txt compiled/triangle.hlsl_lib.spv
spirv-dis.exe -o compiled/triangle.hlsl_rchit.txt compiled/triangle.hlsl_rchit.spv
spirv-dis.exe -o compiled/triangle.hlsl_rgen.txt compiled/triangle.hlsl_rgen.spv
spirv-dis.exe -o compiled/triangle.hlsl_rmiss.txt compiled/triangle.hlsl_rmiss.spv

REM "** DONE **"