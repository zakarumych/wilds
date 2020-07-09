use crate::{
    buffer::Buffer,
    descriptor::DescriptorSet,
    image::{
        Image, ImageBlit, ImageCopy, ImageLayoutTransition, ImageMemoryBarrier,
        Layout,
    },
    pipeline::{
        AccelerationStructureBuildGeometryInfo, GraphicsPipeline,
        PipelineLayout, RayTracingPipeline, ShaderBindingTable, Viewport,
    },
    queue::QueueCapabilityFlags,
    render_pass::{ClearValue, Framebuffer, RenderPass},
    sampler::Filter,
    stage::PipelineStageFlags,
    Extent3d, IndexType, OutOfMemory, Rect2d,
};
use maybe_sync::{MaybeSend, MaybeSync};
use smallvec::SmallVec;
use std::{fmt::Debug, ops::Range};

#[derive(Debug)]
pub enum Command<'a> {
    BeginRenderPass {
        pass: &'a RenderPass,
        framebuffer: &'a Framebuffer,
        clears: &'a [ClearValue],
    },
    EndRenderPass,

    BindGraphicsPipeline {
        pipeline: &'a GraphicsPipeline,
    },

    BindRayTracingPipeline {
        pipeline: &'a RayTracingPipeline,
    },

    BindGraphicsDescriptorSets {
        layout: &'a PipelineLayout,
        first_set: u32,
        sets: &'a [DescriptorSet],
        dynamic_offsets: &'a [u32],
    },

    BindComputeDescriptorSets {
        layout: &'a PipelineLayout,
        first_set: u32,
        sets: &'a [DescriptorSet],
        dynamic_offsets: &'a [u32],
    },

    BindRayTracingDescriptorSets {
        layout: &'a PipelineLayout,
        first_set: u32,
        sets: &'a [DescriptorSet],
        dynamic_offsets: &'a [u32],
    },

    SetViewport {
        viewport: Viewport,
    },

    SetScissor {
        scissor: Rect2d,
    },

    Draw {
        vertices: Range<u32>,
        instances: Range<u32>,
    },

    DrawIndexed {
        indices: Range<u32>,
        vertex_offset: i32,
        instances: Range<u32>,
    },

    UpdateBuffer {
        buffer: &'a Buffer,
        offset: u64,
        data: &'a [u8],
    },

    BindVertexBuffers {
        first: u32,
        buffers: &'a [(Buffer, u64)],
    },

    BindIndexBuffer {
        buffer: &'a Buffer,
        offset: u64,
        index_type: IndexType,
    },

    BuildAccelerationStructure {
        infos: &'a [AccelerationStructureBuildGeometryInfo<'a>],
    },

    TraceRays {
        shader_binding_table: &'a ShaderBindingTable,
        extent: Extent3d,
    },

    CopyImage {
        src_image: &'a Image,
        src_layout: Layout,
        dst_image: &'a Image,
        dst_layout: Layout,
        regions: &'a [ImageCopy],
    },

    BlitImage {
        src_image: &'a Image,
        src_layout: Layout,
        dst_image: &'a Image,
        dst_layout: Layout,
        regions: &'a [ImageBlit],
        filter: Filter,
    },

    PipelineBarrier {
        src: PipelineStageFlags,
        dst: PipelineStageFlags,
        images: &'a [ImageMemoryBarrier<'a>],
    },
}

/// Basis for encoding capabilities.
/// Implements encoding of commands that can be inside and outside of render
/// pass.
#[derive(Debug)]

pub struct EncoderCommon<'a> {
    capabilities: QueueCapabilityFlags,
    commands: Vec<Command<'a>>,
}

impl<'a> EncoderCommon<'a> {
    pub fn set_viewport(&mut self, viewport: Viewport) {
        assert!(self.capabilities.supports_graphics());

        self.commands.push(Command::SetViewport { viewport })
    }

    pub fn set_scissor(&mut self, scissor: Rect2d) {
        assert!(self.capabilities.supports_graphics());

        self.commands.push(Command::SetScissor { scissor })
    }

    pub fn bind_graphics_pipeline(&mut self, pipeline: &'a GraphicsPipeline) {
        assert!(self.capabilities.supports_graphics());

        self.commands
            .push(Command::BindGraphicsPipeline { pipeline })
    }

    // pub fn bind_compute_pipeline(&mut self, pipeline: &'a ComputePipeline) {
    //     assert!(self.capabilities.supports_compute());
    //     self.commands
    //         .push(Command::BindComputePipeline { pipeline })
    // }

    pub fn bind_ray_tracing_pipeline(
        &mut self,
        pipeline: &'a RayTracingPipeline,
    ) {
        assert!(self.capabilities.supports_compute());

        self.commands
            .push(Command::BindRayTracingPipeline { pipeline })
    }

    pub fn bind_vertex_buffers(
        &mut self,
        first: u32,
        buffers: &'a [(Buffer, u64)],
    ) {
        assert!(self.capabilities.supports_graphics());

        self.commands
            .push(Command::BindVertexBuffers { first, buffers })
    }

    pub fn bind_index_buffer(
        &mut self,
        buffer: &'a Buffer,
        offset: u64,
        index_type: IndexType,
    ) {
        assert!(self.capabilities.supports_graphics());

        self.commands.push(Command::BindIndexBuffer {
            buffer,
            offset,
            index_type,
        })
    }

    pub fn bind_graphics_descriptor_sets(
        &mut self,
        layout: &'a PipelineLayout,
        first_set: u32,
        sets: &'a [DescriptorSet],
        dynamic_offsets: &'a [u32],
    ) {
        assert!(self.capabilities.supports_graphics());

        self.commands.push(Command::BindGraphicsDescriptorSets {
            layout,
            first_set,
            sets,
            dynamic_offsets,
        });
    }

    pub fn bind_compute_descriptor_sets(
        &mut self,
        layout: &'a PipelineLayout,
        first_set: u32,
        sets: &'a [DescriptorSet],
        dynamic_offsets: &'a [u32],
    ) {
        assert!(self.capabilities.supports_compute());

        self.commands.push(Command::BindComputeDescriptorSets {
            layout,
            first_set,
            sets,
            dynamic_offsets,
        });
    }

    pub fn bind_ray_tracing_descriptor_sets(
        &mut self,
        layout: &'a PipelineLayout,
        first_set: u32,
        sets: &'a [DescriptorSet],
        dynamic_offsets: &'a [u32],
    ) {
        assert!(self.capabilities.supports_compute());

        self.commands.push(Command::BindRayTracingDescriptorSets {
            layout,
            first_set,
            sets,
            dynamic_offsets,
        });
    }

    pub fn pipeline_barrier(
        &mut self,
        src: PipelineStageFlags,
        dst: PipelineStageFlags,
    ) {
        self.commands.push(Command::PipelineBarrier {
            src,
            dst,
            images: &[],
        });
    }

    pub fn image_barriers(
        &mut self,
        src: PipelineStageFlags,
        dst: PipelineStageFlags,
        images: &'a [ImageMemoryBarrier<'a>],
    ) {
        self.commands
            .push(Command::PipelineBarrier { src, dst, images });
    }
}

/// Command encoder that can encode commands outside render pass.
#[derive(Debug)]

pub struct Encoder<'a> {
    inner: EncoderCommon<'a>,
    command_buffer: Box<dyn CommandBufferTrait>,
}

impl<'a> std::ops::Deref for Encoder<'a> {
    type Target = EncoderCommon<'a>;

    fn deref(&self) -> &EncoderCommon<'a> {
        &self.inner
    }
}

impl<'a> std::ops::DerefMut for Encoder<'a> {
    fn deref_mut(&mut self) -> &mut EncoderCommon<'a> {
        &mut self.inner
    }
}

impl<'a> Encoder<'a> {
    pub(super) fn new(
        command_buffer: Box<dyn CommandBufferTrait>,
        capabilities: QueueCapabilityFlags,
    ) -> Self {
        Encoder {
            inner: EncoderCommon {
                capabilities,
                commands: Vec::new(),
            },
            command_buffer,
        }
    }

    /// Begins render pass and returns `RenderPassEncoder` to encode commands of
    /// the render pass. `RenderPassEncoder` borrows `Encoder`.
    /// To continue use this `Encoder` returned `RenderPassEncoder` must be
    /// dropped which implicitly ends render pass.
    ///
    /// `pass` - render pass to encode.
    /// `framebuffer` - a framebuffer (set of attachments) for render pass to
    /// use. `clears` - an array of clear values.
    ///            render pass will clear attachments with `load_op ==
    /// AttachmentLoadOp::Clear` using those values.            they will be
    /// used in order.

    pub fn with_render_pass(
        &mut self,
        pass: &'a RenderPass,
        framebuffer: &'a Framebuffer,
        clears: &'a [ClearValue],
    ) -> RenderPassEncoder<'_, 'a> {
        assert!(self.inner.capabilities.supports_graphics());

        self.inner.commands.push(Command::BeginRenderPass {
            pass,
            framebuffer,
            clears,
        });

        RenderPassEncoder {
            inner: &mut self.inner,
        }
    }

    /// Updates a buffer's contents from host memory

    pub fn update_buffer<T>(
        &mut self,
        buffer: &'a Buffer,
        offset: u64,
        data: &'a [T],
    ) {
        let data = unsafe {
            std::slice::from_raw_parts(
                data.as_ptr() as *const u8,
                std::mem::size_of_val(data),
            )
        };

        self.inner.commands.push(Command::UpdateBuffer {
            buffer,
            offset,
            data,
        })
    }

    /// Builds acceleration structures.

    pub fn build_acceleration_structure(
        &mut self,
        infos: &'a [AccelerationStructureBuildGeometryInfo<'a>],
    ) {
        assert!(self.inner.capabilities.supports_compute());

        if infos.is_empty() {
            return;
        }

        // Checks.
        for (i, info) in infos.iter().enumerate() {
            if let Some(src) = &info.src {
                for (j, info) in infos[..i].iter().enumerate() {
                    assert_ne!(
                        &info.dst, src,
                        "`infos[{}].src` and `infos[{}].dst` collision",
                        i, j,
                    );
                }
            }

            let dst = &info.dst;

            for (j, info) in infos[..i].iter().enumerate() {
                assert_ne!(
                    info.src.as_ref(),
                    Some(dst),
                    "`infos[{}].src` and `infos[{}].dst` collision",
                    j,
                    i,
                );
            }

            assert!(
                info.geometries.len() <= dst.info().geometries.len(),
                "Wrong number of geometries supplied to build: {}. Acceleration structure has: {}",
                info.geometries.len(),
                dst.info().geometries.len()
            );
        }

        self.inner
            .commands
            .push(Command::BuildAccelerationStructure { infos })
    }

    pub fn trace_rays(
        &mut self,
        shader_binding_table: &'a ShaderBindingTable,
        extent: Extent3d,
    ) {
        assert!(self.inner.capabilities.supports_compute());

        self.commands.push(Command::TraceRays {
            shader_binding_table,
            extent,
        })
    }

    pub fn copy_image(
        &mut self,
        src_image: &'a Image,
        src_layout: Layout,
        dst_image: &'a Image,
        dst_layout: Layout,
        regions: &'a [ImageCopy],
    ) {
        self.commands.push(Command::CopyImage {
            src_image,
            src_layout,
            dst_image,
            dst_layout,
            regions,
        })
    }

    pub fn blit_image(
        &mut self,
        src_image: &'a Image,
        src_layout: Layout,
        dst_image: &'a Image,
        dst_layout: Layout,
        regions: &'a [ImageBlit],
        filter: Filter,
    ) {
        assert!(self.capabilities.supports_graphics());

        self.commands.push(Command::BlitImage {
            src_image,
            src_layout,
            dst_image,
            dst_layout,
            regions,
            filter,
        })
    }

    /// Flushes commands recorded into this encoder to the underlying command
    /// buffer.

    pub fn finish(mut self) -> CommandBuffer {
        self.command_buffer
            .write(&self.inner.commands)
            .expect("TODO: Handle command buffer writing error");

        CommandBuffer {
            inner: self.command_buffer,
        }
    }
}

/// Command encoder that can encode commands inside render pass.
#[derive(Debug)]

pub struct RenderPassEncoder<'a, 'b> {
    inner: &'a mut EncoderCommon<'b>,
}

impl<'a, 'b> RenderPassEncoder<'a, 'b> {
    pub fn draw(&mut self, vertices: Range<u32>, instances: Range<u32>) {
        self.inner.commands.push(Command::Draw {
            vertices,
            instances,
        });
    }

    pub fn draw_indexed(
        &mut self,
        indices: Range<u32>,
        vertex_offset: i32,
        instances: Range<u32>,
    ) {
        self.inner.commands.push(Command::DrawIndexed {
            indices,
            vertex_offset,
            instances,
        });
    }
}

impl Drop for RenderPassEncoder<'_, '_> {
    fn drop(&mut self) {
        self.inner.commands.push(Command::EndRenderPass);
    }
}

impl<'a, 'b> std::ops::Deref for RenderPassEncoder<'a, 'b> {
    type Target = EncoderCommon<'b>;

    fn deref(&self) -> &EncoderCommon<'b> {
        self.inner
    }
}

impl<'a, 'b> std::ops::DerefMut for RenderPassEncoder<'a, 'b> {
    fn deref_mut(&mut self) -> &mut EncoderCommon<'b> {
        self.inner
    }
}

/// Encoded commands submittable to the `Queue`
#[derive(Debug)]
pub struct CommandBuffer {
    inner: Box<dyn CommandBufferTrait>,
}

impl CommandBuffer {
    pub fn downcast<T: 'static>(self) -> Box<T> {
        assert_eq!(self.inner.type_id(), std::any::TypeId::of::<T>());

        unsafe {
            // Relying on `CommandBufferTrait::type_id`.
            // Which is part of `CommandBufferTrait` unsafe contract.
            Box::from_raw(Box::into_raw(self.inner) as *mut T)
        }
    }
}

pub unsafe trait CommandBufferTrait:
    Debug + MaybeSend + MaybeSync + 'static
{
    fn type_id(&self) -> std::any::TypeId;

    /// Write commands into buffer.

    fn write(&mut self, commands: &[Command<'_>]) -> Result<(), OutOfMemory>;
}
