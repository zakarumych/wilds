@echo off

setlocal
cd src/shaders

glslangValidator -V prepass.rchit -o prepass.rchit.spv
glslangValidator -V prepass.rgen -o prepass.rgen.spv
glslangValidator -V prepass.rmiss -o prepass.rmiss.spv
glslangValidator -V shadow.rmiss -o shadow.rmiss.spv
