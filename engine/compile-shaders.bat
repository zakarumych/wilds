@echo off

setlocal
cd src/renderer/pass/rt_prepass

glslangValidator -V prepass.rchit -o prepass.rchit.spv
glslangValidator -V prepass.rgen -o prepass.rgen.spv
glslangValidator -V prepass.rmiss -o prepass.rmiss.spv
glslangValidator -V shadow.rmiss -o shadow.rmiss.spv
