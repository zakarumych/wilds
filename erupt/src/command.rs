use crate::{
    access::supported_access,
    convert::{oom_error_from_erupt, ToErupt},
    device::EruptDevice,
    handle::EruptResource as _,
};
use erupt::{
    extensions::khr_ray_tracing::{
        self as vkrt, KhrRayTracingDeviceLoaderExt as _,
    },
    vk1_0::{self, Vk10DeviceLoaderExt as _},
};
use illume::{
    AccelerationStructureGeometry, AccelerationStructureLevel,
    AttachmentLoadOp, ClearValue, Command, CommandBufferTrait,
    FormatDescription, FormatType, IndexData, IndexType, OutOfMemory, Repr,
    StridedBufferRegion, SMALLVEC_ATTACHMENTS,
};
use smallvec::SmallVec;
use std::{
    convert::TryFrom as _,
    fmt::{self, Debug},
    sync::Weak,
};

pub(super) struct EruptCommandBuffer {
    pub(super) handle: vk1_0::CommandBuffer,
    pub(super) device: Weak<EruptDevice>,
    pub(super) family: u32,
    pub(super) queue: u32,
    pub(super) recording: bool,
}

impl Debug for EruptCommandBuffer {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        if fmt.alternate() {
            fmt.debug_struct("EruptCommandBuffer")
                .field("handle", &self.handle)
                .field("device", &self.device)
                .field("family", &self.family)
                .field("queue", &self.queue)
                .finish()
        } else {
            Debug::fmt(&self.handle, fmt)
        }
    }
}

unsafe impl CommandBufferTrait for EruptCommandBuffer {
    fn type_id(&self) -> std::any::TypeId {
        std::any::TypeId::of::<Self>()
    }

    fn write(&mut self, commands: &[Command<'_>]) -> Result<(), OutOfMemory> {
        let device = match self.device.upgrade() {
            Some(device) => device,
            None => return Ok(()),
        };

        if !self.recording {
            unsafe {
                device.logical.begin_command_buffer(
                    self.handle,
                    &vk1_0::CommandBufferBeginInfo::default()
                        .builder()
                        .flags(vk1_0::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                )
            }
            .result()
            .map_err(oom_error_from_erupt)?;

            self.recording = true;
        }

        let logical = &device.logical;

        for command in commands {
            match command {
                &Command::BeginRenderPass {
                    pass,
                    framebuffer,
                    clears,
                } => {
                    let clear_values = pass
                            .info()
                            .attachments
                            .iter()
                            .filter(|a| a.load_op == AttachmentLoadOp::Clear)
                            .zip(clears)
                            .map(|(attachment, clear)| {
                                use FormatDescription::*;
                                match clear {
                                    &ClearValue::Color(r, g, b, a) => vk1_0::ClearValue {
                                    color: match attachment.format.description() {
                                        R(repr)|RG(repr)|RGB(repr)|RGBA(repr)|BGR(repr)|BGRA(repr) => colors_f32_to_value(r, g, b, a, repr),
                                        _ => panic!("Attempt to clear depth-stencil attachment with color value"),
                                    }
                                },
                                &ClearValue::DepthStencil(depth, stencil) => {
                                    assert!(
                                        attachment.format.is_depth()
                                            || attachment.format.is_stencil()
                                    );
                                    vk1_0::ClearValue {
                                        depth_stencil: vk1_0::ClearDepthStencilValue {
                                            depth,
                                            stencil,
                                        },
                                    }
                                }}
                            })
                            .collect::<SmallVec<[_; SMALLVEC_ATTACHMENTS]>>();

                    unsafe {
                        logical.cmd_begin_render_pass(
                            self.handle,
                            &vk1_0::RenderPassBeginInfo::default()
                                .builder()
                                .render_pass(pass.erupt_ref(&*device).handle)
                                .framebuffer(
                                    framebuffer.erupt_ref(&*device).handle,
                                ) //FIXME: Check `framebuffer` belongs to the
                                // pass.
                                .render_area(vk1_0::Rect2D {
                                    offset: vk1_0::Offset2D { x: 0, y: 0 },
                                    extent: framebuffer
                                        .info()
                                        .extent
                                        .to_erupt(),
                                })
                                .clear_values(&clear_values),
                            vk1_0::SubpassContents::INLINE,
                        )
                    }
                }
                Command::EndRenderPass => unsafe {
                    logical.cmd_end_render_pass(self.handle)
                },
                &Command::BindGraphicsPipeline { pipeline } => unsafe {
                    logical.cmd_bind_pipeline(
                        self.handle,
                        vk1_0::PipelineBindPoint::GRAPHICS,
                        pipeline.erupt_ref(&*device).handle,
                    )
                },
                Command::Draw {
                    vertices,
                    instances,
                } => unsafe {
                    logical.cmd_draw(
                        self.handle,
                        vertices.end - vertices.start,
                        instances.end - instances.start,
                        vertices.start,
                        instances.start,
                    )
                },
                Command::DrawIndexed {
                    indices,
                    vertex_offset,
                    instances,
                } => unsafe {
                    logical.cmd_draw_indexed(
                        self.handle,
                        indices.end - indices.start,
                        instances.end - instances.start,
                        indices.start,
                        *vertex_offset,
                        instances.start,
                    )
                },
                Command::SetViewport { viewport } => unsafe {
                    // FIXME: Check that bound pipeline has dynamic viewport
                    // state.
                    logical.cmd_set_viewport(
                        self.handle,
                        0,
                        &[viewport.to_erupt().builder()],
                    );
                },
                Command::SetScissor { scissor } => unsafe {
                    // FIXME: Check that bound pipeline has dynamic scissor
                    // state.
                    logical.cmd_set_scissor(
                        self.handle,
                        0,
                        &[scissor.to_erupt().builder()],
                    );
                },
                &Command::UpdateBuffer {
                    buffer,
                    offset,
                    data,
                } => unsafe {
                    assert_eq!(offset % 4, 0);

                    assert!(data.len() < 65_536);

                    logical.cmd_update_buffer(
                        self.handle,
                        buffer.erupt_ref(&*device).handle,
                        offset,
                        data.len() as _,
                        data.as_ptr() as _,
                    );
                },
                Command::BindVertexBuffers { first, buffers } => unsafe {
                    let offsets: SmallVec<[_; 8]> =
                        buffers.iter().map(|&(_, offset)| offset).collect();

                    let buffers: SmallVec<[_; 8]> = buffers
                        .iter()
                        .map(|(buffer, _)| buffer.erupt_ref(&*device).handle)
                        .collect();

                    logical.cmd_bind_vertex_buffers(
                        self.handle,
                        *first,
                        &buffers,
                        &offsets,
                    );
                },
                &Command::BuildAccelerationStructure { infos } => {
                    assert!(device.logical.khr_ray_tracing.is_some());

                    // Vulkan specific checks.
                    assert!(
                        u32::try_from(infos.len()).is_ok(),
                        "Too many infos"
                    );

                    for (i, info) in infos.iter().enumerate() {
                        if let Some(src) = &info.src {
                            assert!(
                                src.is_owner(&*device),
                                "`infos[{}].src` belongs to wrong device",
                                i
                            );
                        }

                        let dst = &info.dst;

                        assert!(
                            dst.is_owner(&*device),
                            "`infos[{}].dst` belongs to wrong device",
                            i,
                        );
                    }

                    // Collect geometries.
                    let mut geometries = SmallVec::<[_; 32]>::new();

                    let mut offsets = SmallVec::<[_; 32]>::new();

                    let ranges: SmallVec<[_; 32]> = infos.iter().map(|info| {
                            let mut total_primitive_count = 0u64;
                            let offset = geometries.len();
                            for geometry in info.geometries {
                                match geometry {
                                    AccelerationStructureGeometry::Triangles {
                                        flags,
                                        vertex_format,
                                        vertex_data,
                                        vertex_stride,
                                        first_vertex,
                                        primitive_count,
                                        index_data,
                                        transform_data,
                                    } => {
                                        total_primitive_count += (*primitive_count) as u64;
                                        geometries.push(vkrt::AccelerationStructureGeometryKHR::default().builder()
                                            .flags(flags.to_erupt())
                                            .geometry_type(vkrt::GeometryTypeKHR::TRIANGLES_KHR)
                                            .geometry(vkrt::AccelerationStructureGeometryDataKHR {
                                                triangles: unsafe {vkrt::AccelerationStructureGeometryTrianglesDataKHR::default().builder()
                                                .vertex_format(vertex_format.to_erupt())
                                                .vertex_data(vertex_data.to_erupt())
                                                .vertex_stride(*vertex_stride)
                                                .index_type(match index_data {
                                                    None => vk1_0::IndexType::NONE_KHR,
                                                    Some(IndexData::U16(_)) => vk1_0::IndexType::UINT16,
                                                    Some(IndexData::U32(_)) => vk1_0::IndexType::UINT32,
                                                })
                                                .index_data(match index_data {
                                                    None => Default::default(),
                                                    Some(IndexData::U16(device_address)) => device_address.to_erupt(),
                                                    Some(IndexData::U32(device_address)) => device_address.to_erupt(),
                                                })
                                                .transform_data(transform_data.as_ref().map(|da| da.to_erupt()).unwrap_or_default())
                                                .discard()}
                                            }));

                                        offsets.push(vkrt::AccelerationStructureBuildOffsetInfoKHR::default().builder()
                                            .primitive_count(*primitive_count)
                                            .first_vertex(*first_vertex)
                                        );
                                    }
                                    AccelerationStructureGeometry::AABBs { flags, data, stride, primitive_count } => {
                                        total_primitive_count += (*primitive_count) as u64;
                                        geometries.push(vkrt::AccelerationStructureGeometryKHR::default().builder()
                                            .flags(flags.to_erupt())
                                            .geometry_type(vkrt::GeometryTypeKHR::AABBS_KHR)
                                            .geometry(vkrt::AccelerationStructureGeometryDataKHR {
                                                aabbs: unsafe {vkrt::AccelerationStructureGeometryAabbsDataKHR::default().builder()
                                                    .data(data.to_erupt())
                                                    .stride(*stride)
                                                    .discard()}
                                            }));

                                        offsets.push(vkrt::AccelerationStructureBuildOffsetInfoKHR::default().builder()
                                            .primitive_count(*primitive_count)
                                        );
                                    }
                                    AccelerationStructureGeometry::Instances { flags, data, primitive_count } => {
                                        geometries.push(vkrt::AccelerationStructureGeometryKHR::default().builder()
                                            .flags(flags.to_erupt())
                                            .geometry_type(vkrt::GeometryTypeKHR::INSTANCES_KHR)
                                            .geometry(vkrt::AccelerationStructureGeometryDataKHR {
                                                instances: unsafe{vkrt::AccelerationStructureGeometryInstancesDataKHR::default().builder()
                                                    .data(data.to_erupt())
                                                    .discard()}
                                            }));

                                        offsets.push(vkrt::AccelerationStructureBuildOffsetInfoKHR::default().builder()
                                            .primitive_count(*primitive_count)
                                        );
                                    }
                                }
                            }

                            if let AccelerationStructureLevel::Bottom = info.dst.info().level {
                                assert!(total_primitive_count <= device.properties.rt.max_primitive_count);
                            }

                            offset .. geometries.len()
                        }).collect();

                    let geometries_pointers: SmallVec<[_; 32]> = ranges
                        .iter()
                        .map(|range| &*geometries[range.start])
                        .collect();

                    let build_infos: SmallVec<[_; 32]> = infos
                        .iter()
                        .zip(&geometries_pointers)
                        .zip(&ranges)
                        .map(|((info, geometry_pointer), range)| {
                            let src = info
                                .src
                                .as_ref()
                                .map(|src| src.erupt_ref(&*device).handle)
                                .unwrap_or_default();
                            let dst = info.dst.erupt_ref(&*device).handle;

                            let dst_info = info.dst.info();
                            vkrt::AccelerationStructureBuildGeometryInfoKHR::default()
                                .builder()
                                ._type(dst_info.level.to_erupt())
                                .flags(dst_info.flags.to_erupt())
                                .update(info.src.is_some())
                                .src_acceleration_structure(src)
                                .dst_acceleration_structure(dst)
                                .geometry_array_of_pointers(false)
                                .geometry_count(range.len() as u32)
                                .geometries(geometry_pointer)
                                .scratch_data(info.scratch.to_erupt())
                        })
                        .collect();

                    let build_offsets: SmallVec<[_; 32]> = ranges
                        .into_iter()
                        .map(|range| &offsets[range][0] as *const _)
                        .collect();

                    unsafe {
                        device.logical.cmd_build_acceleration_structure_khr(
                            self.handle,
                            &build_infos,
                            &build_offsets,
                        )
                    }
                }
                &Command::BindIndexBuffer {
                    buffer,
                    offset,
                    index_type,
                } => unsafe {
                    logical.cmd_bind_index_buffer(
                        self.handle,
                        buffer.erupt_ref(&*device).handle,
                        offset,
                        match index_type {
                            IndexType::U16 => vk1_0::IndexType::UINT16,
                            IndexType::U32 => vk1_0::IndexType::UINT32,
                        },
                    );
                },

                &Command::BindRayTracingPipeline { pipeline } => unsafe {
                    logical.cmd_bind_pipeline(
                        self.handle,
                        vk1_0::PipelineBindPoint::RAY_TRACING_KHR,
                        pipeline.erupt_ref(&*device).handle,
                    )
                },

                &Command::BindGraphicsDescriptorSets {
                    layout,
                    first_set,
                    sets,
                    dynamic_offsets,
                } => unsafe {
                    logical.cmd_bind_descriptor_sets(
                        self.handle,
                        vk1_0::PipelineBindPoint::GRAPHICS,
                        layout.erupt_ref(&*device).handle,
                        first_set,
                        &sets
                            .iter()
                            .map(|set| set.erupt_ref(&*device).handle)
                            .collect::<SmallVec<[_; 8]>>(),
                        dynamic_offsets,
                    )
                },

                &Command::BindComputeDescriptorSets {
                    layout,
                    first_set,
                    sets,
                    dynamic_offsets,
                } => unsafe {
                    logical.cmd_bind_descriptor_sets(
                        self.handle,
                        vk1_0::PipelineBindPoint::COMPUTE,
                        layout.erupt_ref(&*device).handle,
                        first_set,
                        &sets
                            .iter()
                            .map(|set| set.erupt_ref(&*device).handle)
                            .collect::<SmallVec<[_; 8]>>(),
                        dynamic_offsets,
                    )
                },

                &Command::BindRayTracingDescriptorSets {
                    layout,
                    first_set,
                    sets,
                    dynamic_offsets,
                } => unsafe {
                    logical.cmd_bind_descriptor_sets(
                        self.handle,
                        vk1_0::PipelineBindPoint::RAY_TRACING_KHR,
                        layout.erupt_ref(&*device).handle,
                        first_set,
                        &sets
                            .iter()
                            .map(|set| set.erupt_ref(&*device).handle)
                            .collect::<SmallVec<[_; 8]>>(),
                        dynamic_offsets,
                    )
                },

                &Command::TraceRays {
                    shader_binding_table,
                    extent,
                } => {
                    assert!(device.logical.khr_ray_tracing.is_some());

                    let sbr = vkrt::StridedBufferRegionKHR::default()
                        .builder()
                        .buffer(vk1_0::Buffer::null());

                    let to_erupt = |sbr: &StridedBufferRegion| {
                        vkrt::StridedBufferRegionKHR {
                            buffer: sbr.buffer.erupt_ref(&*device).handle,
                            offset: sbr.offset,
                            size: sbr.size,
                            stride: sbr.stride,
                        }
                    };

                    unsafe {
                        device.logical.cmd_trace_rays_khr(
                            self.handle,
                            &shader_binding_table
                                .raygen
                                .as_ref()
                                .map_or(*sbr, to_erupt),
                            &shader_binding_table
                                .miss
                                .as_ref()
                                .map_or(*sbr, to_erupt),
                            &shader_binding_table
                                .hit
                                .as_ref()
                                .map_or(*sbr, to_erupt),
                            &shader_binding_table
                                .callable
                                .as_ref()
                                .map_or(*sbr, to_erupt),
                            extent.width,
                            extent.height,
                            extent.depth,
                        )
                    }
                }
                &Command::CopyImage {
                    src_image,
                    src_layout,
                    dst_image,
                    dst_layout,
                    regions,
                } => unsafe {
                    logical.cmd_copy_image(
                        self.handle,
                        src_image.erupt_ref(&*device).handle,
                        src_layout.to_erupt(),
                        dst_image.erupt_ref(&*device).handle,
                        dst_layout.to_erupt(),
                        &regions
                            .iter()
                            .map(|region| region.to_erupt().builder())
                            .collect::<SmallVec<[_; 4]>>(),
                    );
                },

                &Command::BlitImage {
                    src_image,
                    src_layout,
                    dst_image,
                    dst_layout,
                    regions,
                    filter,
                } => unsafe {
                    logical.cmd_blit_image(
                        self.handle,
                        src_image.erupt_ref(&*device).handle,
                        src_layout.to_erupt(),
                        dst_image.erupt_ref(&*device).handle,
                        dst_layout.to_erupt(),
                        &regions
                            .iter()
                            .map(|region| region.to_erupt().builder())
                            .collect::<SmallVec<[_; 4]>>(),
                        filter.to_erupt(),
                    );
                },

                &Command::PipelineBarrier { src, dst, images } => unsafe {
                    logical.cmd_pipeline_barrier(
                        self.handle,
                        src.to_erupt(),
                        dst.to_erupt(),
                        vk1_0::DependencyFlags::empty(),
                        &[vk1_0::MemoryBarrier::default()
                            .builder()
                            .src_access_mask(supported_access(src.to_erupt()))
                            .dst_access_mask(supported_access(dst.to_erupt()))],
                        &[],
                        &images
                            .iter()
                            .map(|image| {
                                vk1_0::ImageMemoryBarrier::default()
                                    .builder()
                                    .image(
                                        image.image.erupt_ref(&*device).handle,
                                    )
                                    .src_access_mask(supported_access(
                                        src.to_erupt(),
                                    ))
                                    .dst_access_mask(supported_access(
                                        dst.to_erupt(),
                                    ))
                                    .old_layout(image.old_layout.to_erupt())
                                    .new_layout(image.new_layout.to_erupt())
                                    .src_queue_family_index(
                                        image
                                            .family_transfer
                                            .as_ref()
                                            .map(|r| r.start)
                                            .unwrap_or(
                                                vk1_0::QUEUE_FAMILY_IGNORED,
                                            ),
                                    )
                                    .dst_queue_family_index(
                                        image
                                            .family_transfer
                                            .as_ref()
                                            .map(|r| r.end)
                                            .unwrap_or(
                                                vk1_0::QUEUE_FAMILY_IGNORED,
                                            ),
                                    )
                                    .subresource_range(
                                        image.subresource.to_erupt(),
                                    )
                            })
                            .collect::<SmallVec<[_; 8]>>(),
                    )
                },
            }
        }

        unsafe { logical.end_command_buffer(self.handle) }
            .result()
            .map_err(oom_error_from_erupt)?;

        Ok(())
    }
}

fn color_f32_to_uint64(color: f32) -> u64 {
    color.min(0f32).max(u64::max_value() as f32) as u64
}

fn color_f32_to_sint64(color: f32) -> i64 {
    color
        .min(i64::min_value() as f32)
        .max(i64::max_value() as f32) as i64
}

fn color_f32_to_uint32(color: f32) -> u32 {
    color.min(0f32).max(u32::max_value() as f32) as u32
}

fn color_f32_to_sint32(color: f32) -> i32 {
    color
        .min(i32::min_value() as f32)
        .max(i32::max_value() as f32) as i32
}

fn color_f32_to_uint16(color: f32) -> u16 {
    color.min(0f32).max(u16::max_value() as f32) as u16
}

fn color_f32_to_sint16(color: f32) -> i16 {
    color
        .min(i16::min_value() as f32)
        .max(i16::max_value() as f32) as i16
}

fn color_f32_to_uint8(color: f32) -> u8 {
    color.min(0f32).max(u8::max_value() as f32) as u8
}

fn color_f32_to_sint8(color: f32) -> i8 {
    color
        .min(i8::min_value() as f32)
        .max(i8::max_value() as f32) as i8
}

fn colors_f32_to_value(
    r: f32,
    g: f32,
    b: f32,
    a: f32,
    repr: Repr,
) -> vk1_0::ClearColorValue {
    match repr {
        Repr {
            bits: 8,
            ty: FormatType::Uint,
        } => vk1_0::ClearColorValue {
            uint32: [
                color_f32_to_uint8(r) as _,
                color_f32_to_uint8(g) as _,
                color_f32_to_uint8(b) as _,
                color_f32_to_uint8(a) as _,
            ],
        },
        Repr {
            bits: 8,
            ty: FormatType::Sint,
        } => vk1_0::ClearColorValue {
            int32: [
                color_f32_to_sint8(r) as _,
                color_f32_to_sint8(g) as _,
                color_f32_to_sint8(b) as _,
                color_f32_to_sint8(a) as _,
            ],
        },
        Repr {
            bits: 16,
            ty: FormatType::Uint,
        } => vk1_0::ClearColorValue {
            uint32: [
                color_f32_to_uint16(r) as _,
                color_f32_to_uint16(g) as _,
                color_f32_to_uint16(b) as _,
                color_f32_to_uint16(a) as _,
            ],
        },
        Repr {
            bits: 16,
            ty: FormatType::Sint,
        } => vk1_0::ClearColorValue {
            int32: [
                color_f32_to_sint16(r) as _,
                color_f32_to_sint16(g) as _,
                color_f32_to_sint16(b) as _,
                color_f32_to_sint16(a) as _,
            ],
        },
        Repr {
            bits: 32,
            ty: FormatType::Uint,
        } => vk1_0::ClearColorValue {
            uint32: [
                color_f32_to_uint32(r) as _,
                color_f32_to_uint32(g) as _,
                color_f32_to_uint32(b) as _,
                color_f32_to_uint32(a) as _,
            ],
        },
        Repr {
            bits: 32,
            ty: FormatType::Sint,
        } => vk1_0::ClearColorValue {
            int32: [
                color_f32_to_sint32(r) as _,
                color_f32_to_sint32(g) as _,
                color_f32_to_sint32(b) as _,
                color_f32_to_sint32(a) as _,
            ],
        },
        Repr {
            bits: 64,
            ty: FormatType::Uint,
        } => vk1_0::ClearColorValue {
            uint32: [
                color_f32_to_uint64(r) as _,
                color_f32_to_uint64(g) as _,
                color_f32_to_uint64(b) as _,
                color_f32_to_uint64(a) as _,
            ],
        },
        Repr {
            bits: 64,
            ty: FormatType::Sint,
        } => vk1_0::ClearColorValue {
            int32: [
                color_f32_to_sint64(r) as _,
                color_f32_to_sint64(g) as _,
                color_f32_to_sint64(b) as _,
                color_f32_to_sint64(a) as _,
            ],
        },
        _ => vk1_0::ClearColorValue {
            float32: [r, g, b, a],
        },
    }
}
