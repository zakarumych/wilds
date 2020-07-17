@echo off

setlocal
cd src/renderer/pass/rt_prepass

glslangValidator -V primary.rchit -o primary.rchit.spv
glslangValidator -V primary.rgen -o primary.rgen.spv
glslangValidator -V primary.rmiss -o primary.rmiss.spv

glslangValidator -V diffuse.rchit -o diffuse.rchit.spv
glslangValidator -V diffuse.rmiss -o diffuse.rmiss.spv

glslangValidator -V shadow.rmiss -o shadow.rmiss.spv
