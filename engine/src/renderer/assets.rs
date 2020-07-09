use crate::{
    descriptor::{
        DescriptorSet, DescriptorSetInfo, DescriptorSetLayout,
        DescriptorSetLayoutInfo,
    },
    device::{CreateRenderPassError, Device},
    fence::{Fence, FenceInfo},
    pipeline::{
        Bounds, ColorBlend, Culling, DepthTest, FrontFace, GraphicsPipeline,
        GraphicsPipelineInfo, PipelineLayoutInfo, PolygonMode,
        PrimitiveTopology, Rasterizer, State, StencilTests,
        VertexInputAttribute, VertexInputBinding, Viewport,
    },
    render_pass::{RenderPass, RenderPassInfo},
    sampler::{Sampler, SamplerInfo},
    shader::{
        AnyHitShader, ClosestHitShader, ComputeShader, CreateShaderModuleError,
        FragmentShader, GeometryShader, IntersectionShader, MissShader,
        RaygenShader, Shader, ShaderLanguage, ShaderModule, ShaderModuleInfo,
        TessellationControlShader, TessellationEvaluationShader, VertexShader,
    },
    OutOfMemory, Rect2d,
};
use goods::*;
use std::{convert::Infallible, future::Future, pin::Pin};

#[derive(Debug)]
pub struct GraphicsPipelineRepr {
    desc: GraphicsPipelineDesc<ShaderRepr>,
    render_pass: RenderPass,
    subpass: u32,
}

impl SyncAsset for GraphicsPipeline {
    type Context = Device;
    type Error = CreateShaderModuleError;
    type Repr = GraphicsPipelineRepr;

    fn build(
        repr: GraphicsPipelineRepr,
        device: &mut Device,
    ) -> Result<Self, CreateShaderModuleError> {
        let desc = repr.desc;

        let layout = device.create_pipeline_layout(PipelineLayoutInfo {
            sets: desc
                .layout
                .sets
                .into_iter()
                .map(|set| -> Result<_, OutOfMemory> {
                    device.create_descriptor_set_layout(set)
                })
                .collect::<Result<_, _>>()?,
        })?;

        let pipeline =
            device.create_graphics_pipeline(GraphicsPipelineInfo {
                vertex_bindings: desc.vertex_bindings,
                vertex_attributes: desc.vertex_attributes,
                primitive_topology: desc.primitive_topology,
                primitive_restart_enable: desc.primitive_restart_enable,
                vertex_shader: VertexShader::new(
                    device.create_shader_module(ShaderModuleInfo {
                        code: desc.vertex_shader.code,
                        language: desc.vertex_shader.language,
                    })?,
                    desc.vertex_shader.entry,
                ),
                rasterizer: if let Some(desc) = desc.rasterizer {
                    Some(Rasterizer {
                        fragment_shader: if let Some(shader) =
                            desc.fragment_shader
                        {
                            Some(FragmentShader::new(
                                device.create_shader_module(
                                    ShaderModuleInfo {
                                        code: shader.code,
                                        language: shader.language,
                                    },
                                )?,
                                shader.entry,
                            ))
                        } else {
                            None
                        },
                        viewport: desc.viewport,
                        scissor: desc.scissor,
                        depth_clamp: desc.depth_clamp,
                        front_face: desc.front_face,
                        culling: desc.culling,
                        polygon_mode: desc.polygon_mode,
                        depth_test: desc.depth_test,
                        stencil_tests: desc.stencil_tests,
                        depth_bounds: desc.depth_bounds,
                        color_blend: desc.color_blend,
                    })
                } else {
                    None
                },
                layout,
                render_pass: repr.render_pass,
                subpass: repr.subpass,
            })?;

        Ok(pipeline)
    }
}

#[derive(Debug)]
pub struct ShaderRepr {
    code: Box<[u8]>,
    language: ShaderLanguage,
    entry: Box<str>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum ShaderDesc<K> {
    External {
        source: K,

        #[serde(skip_serializing_if = "entry_is_main", default = "entry_main")]
        entry: Box<str>,
        language: ShaderLanguage,
    },
    Inlined {
        #[serde(with = "serde_bytes")]
        code: Box<[u8]>,

        #[serde(skip_serializing_if = "entry_is_main", default = "entry_main")]
        entry: Box<str>,
        language: ShaderLanguage,
    },
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct PipelineLayoutDesc {
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub sets: Vec<DescriptorSetLayoutInfo>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct RasterizerDesc<S> {
    #[serde(
        skip_serializing_if = "State::is_dynamic",
        default = "State::dynamic"
    )]
    pub viewport: State<Viewport>,
    #[serde(
        skip_serializing_if = "State::is_dynamic",
        default = "State::dynamic"
    )]
    pub scissor: State<Rect2d>,
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub depth_clamp: bool,
    #[serde(skip_serializing_if = "is_default", default)]
    pub front_face: FrontFace,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub culling: Option<Culling>,
    #[serde(skip_serializing_if = "is_default", default)]
    pub polygon_mode: PolygonMode,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub depth_test: Option<DepthTest>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub stencil_tests: Option<StencilTests>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub depth_bounds: Option<State<Bounds>>,
    #[serde(skip_serializing_if = "Option::is_none", default = "none")]
    pub fragment_shader: Option<S>,
    #[serde(skip_serializing_if = "is_default", default)]
    pub color_blend: ColorBlend,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct GraphicsPipelineDesc<S> {
    pub vertex_shader: S,
    pub layout: PipelineLayoutDesc,

    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub vertex_bindings: Vec<VertexInputBinding>,

    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub vertex_attributes: Vec<VertexInputAttribute>,

    #[serde(skip_serializing_if = "is_default", default)]
    pub primitive_topology: PrimitiveTopology,

    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub primitive_restart_enable: bool,

    #[serde(
        skip_serializing_if = "Option::is_none",
        default = "none",
        flatten
    )]
    pub rasterizer: Option<RasterizerDesc<S>>,
}

#[derive(Clone, Debug)]
pub struct GraphicsPipelineRonFormat {
    render_pass: RenderPass,
    subpass: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum GraphicsPipelineRonFormatError {
    #[error("Failed to decode GraphicsPipelineDesc from RON: {source}")]
    Ron {
        #[from]
        source: ron::de::Error,
    },

    #[error("Failed to load shaders: {source}")]
    ShaderLoad {
        #[from]
        source: Error<Box<[u8]>>,
    },
}

impl<K> Format<GraphicsPipeline, K> for GraphicsPipelineRonFormat
where
    K: serde::de::DeserializeOwned + Key,
{
    type DecodeFuture = Pin<
        Box<
            dyn Future<
                    Output = Result<
                        GraphicsPipelineRepr,
                        GraphicsPipelineRonFormatError,
                    >,
                > + Send,
        >,
    >;
    type Error = GraphicsPipelineRonFormatError;

    fn decode(self, bytes: Vec<u8>, cache: &Cache<K>) -> Self::DecodeFuture {
        match ron::de::from_bytes::<GraphicsPipelineDesc<ShaderDesc<K>>>(&bytes)
        {
            Ok(pipeline) => {
                let mut vertex_shader_code =
                    if let ShaderDesc::External { source, .. } =
                        &pipeline.vertex_shader
                    {
                        Some(cache.load(source.clone()))
                    } else {
                        None
                    };

                let mut fragment_shader_code =
                    if let Some(rasterizer) = &pipeline.rasterizer {
                        if let Some(ShaderDesc::External { source, .. }) =
                            &rasterizer.fragment_shader
                        {
                            Some(cache.load(source.clone()))
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                Box::pin(async move {
                    Ok(GraphicsPipelineRepr {
                        desc: GraphicsPipelineDesc {
                            vertex_bindings: pipeline.vertex_bindings,
                            vertex_attributes: pipeline.vertex_attributes,
                            primitive_topology: pipeline.primitive_topology,
                            primitive_restart_enable: pipeline
                                .primitive_restart_enable,
                            vertex_shader: match pipeline.vertex_shader {
                                ShaderDesc::External {
                                    language,
                                    entry,
                                    ..
                                } => ShaderRepr {
                                    code: vertex_shader_code
                                        .take()
                                        .unwrap()
                                        .await?,
                                    entry,
                                    language,
                                },
                                ShaderDesc::Inlined {
                                    code,
                                    language,
                                    entry,
                                } => ShaderRepr {
                                    code,
                                    language,
                                    entry,
                                },
                            },
                            rasterizer: if let Some(rasterizer) =
                                pipeline.rasterizer
                            {
                                Some(RasterizerDesc {
                                    viewport: rasterizer.viewport,
                                    scissor: rasterizer.scissor,
                                    depth_clamp: rasterizer.depth_clamp,
                                    front_face: rasterizer.front_face,
                                    culling: rasterizer.culling,
                                    polygon_mode: rasterizer.polygon_mode,
                                    depth_test: rasterizer.depth_test,
                                    stencil_tests: rasterizer.stencil_tests,
                                    depth_bounds: rasterizer.depth_bounds,
                                    fragment_shader: if let Some(
                                        fragment_shader,
                                    ) =
                                        rasterizer.fragment_shader
                                    {
                                        Some(match fragment_shader {
                                            ShaderDesc::External {
                                                language,
                                                entry,
                                                ..
                                            } => ShaderRepr {
                                                code: fragment_shader_code
                                                    .take()
                                                    .unwrap()
                                                    .await?,
                                                entry,
                                                language,
                                            },
                                            ShaderDesc::Inlined {
                                                code,
                                                language,
                                                entry,
                                            } => ShaderRepr {
                                                code,
                                                language,
                                                entry,
                                            },
                                        })
                                    } else {
                                        None
                                    },
                                    color_blend: rasterizer.color_blend,
                                })
                            } else {
                                None
                            },
                            layout: pipeline.layout,
                        },
                        render_pass: self.render_pass,
                        subpass: self.subpass,
                    })
                })
            }
            Err(err) => Box::pin(async move { Err(err.into()) }),
        }
    }
}

fn none<T>() -> Option<T> {
    None
}

fn is_default<T: Default + Eq>(value: &T) -> bool {
    *value == T::default()
}

fn entry_main() -> Box<str> {
    "main".into()
}

fn entry_is_main(entry: &str) -> bool {
    entry == "main"
}
