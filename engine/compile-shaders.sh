cd src/renderer/pass

glslangValidator -V rt_prepass/primary.rchit -o rt_prepass/primary.rchit.spv
glslangValidator -V rt_prepass/primary.rgen -o rt_prepass/primary.rgen.spv
glslangValidator -V rt_prepass/primary.rmiss -o rt_prepass/primary.rmiss.spv
glslangValidator -V rt_prepass/diffuse.rchit -o rt_prepass/diffuse.rchit.spv
glslangValidator -V rt_prepass/diffuse.rmiss -o rt_prepass/diffuse.rmiss.spv
glslangValidator -V rt_prepass/shadow.rmiss -o rt_prepass/shadow.rmiss.spv

glslangValidator -V combine/combine.vert -o combine/combine.vert.spv
glslangValidator -V combine/combine.frag -o combine/combine.frag.spv

glslangValidator -V diffuse_filter/diffuse_filter.vert -o diffuse_filter/diffuse_filter.vert.spv
glslangValidator -V diffuse_filter/diffuse_filter.frag -o diffuse_filter/diffuse_filter.frag.spv
