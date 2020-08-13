@echo off

setlocal
cd src/renderer/pass

glslangValidator -V rt_prepass/primary.rchit -o rt_prepass/primary.rchit.spv
glslangValidator -V rt_prepass/primary.rgen -o rt_prepass/primary.rgen.spv
glslangValidator -V rt_prepass/primary.rmiss -o rt_prepass/primary.rmiss.spv
glslangValidator -V rt_prepass/diffuse.rchit -o rt_prepass/diffuse.rchit.spv
glslangValidator -V rt_prepass/diffuse.rmiss -o rt_prepass/diffuse.rmiss.spv
glslangValidator -V rt_prepass/shadow.rmiss -o rt_prepass/shadow.rmiss.spv

glslangValidator -V combine/combine.vert -o combine/combine.vert.spv
glslangValidator -V combine/combine.frag -o combine/combine.frag.spv

glslangValidator -V gauss_filter/gauss_filter.vert -o gauss_filter/gauss_filter.vert.spv
glslangValidator -V gauss_filter/gauss_filter.frag -o gauss_filter/gauss_filter.frag.spv

glslangValidator -V atrous/atrous.vert -o atrous/atrous.vert.spv
glslangValidator -V atrous/atrous0h.frag -o atrous/atrous0h.frag.spv
glslangValidator -V atrous/atrous1h.frag -o atrous/atrous1h.frag.spv
glslangValidator -V atrous/atrous2h.frag -o atrous/atrous2h.frag.spv
glslangValidator -V atrous/atrous0v.frag -o atrous/atrous0v.frag.spv
glslangValidator -V atrous/atrous1v.frag -o atrous/atrous1v.frag.spv
glslangValidator -V atrous/atrous2v.frag -o atrous/atrous2v.frag.spv
