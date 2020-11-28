//!
//! Frame-pass that transforms vertices of mesh into specified pose.

use {
    super::{Pass, SparseDescriptors},
    crate::{
        animate::Pose,
        renderer::{
            Context, Mesh, PoseMesh, PositionNormalTangent3dUV, Renderable,
            Skin, VertexType,
        },
    },
    bumpalo::{collections::Vec as BVec, Bump},
    bytemuck::{Pod, Zeroable},
    eyre::Report,
    hecs::World,
    illume::{
        BufferInfo, BufferUsage, ComputePipeline, ComputePipelineInfo,
        ComputeShader, DescriptorBindingFlags, DescriptorSet,
        DescriptorSetInfo, DescriptorSetLayoutBinding,
        DescriptorSetLayoutFlags, DescriptorSetLayoutInfo, DescriptorType,
        Descriptors, Fence, MappableBuffer, MemoryUsage, OutOfMemory,
        PipelineLayout, PipelineLayoutInfo, PipelineStageFlags, PushConstant,
        Semaphore, ShaderStageFlags, Spirv, WriteDescriptorSet,
    },
    nalgebra as na,
    std::{convert::TryInto as _, mem::size_of_val},
};

pub struct PosePass {
    layout: PipelineLayout,
    pipeline: ComputePipeline,
    set: DescriptorSet,
    per_frame_sets: [DescriptorSet; 2],
    meshes: SparseDescriptors<Mesh>,
    joints_buffer: Option<MappableBuffer>,
    joints_buffer_written: [bool; 2],
}

impl PosePass {
    pub fn new(ctx: &mut Context) -> Result<Self, Report> {
        let set_layout = ctx.create_descriptor_set_layout(DescriptorSetLayoutInfo {
            flags: DescriptorSetLayoutFlags::empty(),
            bindings: vec![
                DescriptorSetLayoutBinding {
                    binding: 0,
                    ty: DescriptorType::StorageBuffer,
                    count: 1024,
                    stages: ShaderStageFlags::COMPUTE,
                    flags: DescriptorBindingFlags::PARTIALLY_BOUND | DescriptorBindingFlags::UPDATE_UNUSED_WHILE_PENDING,
                },
                DescriptorSetLayoutBinding {
                    binding: 1,
                    ty: DescriptorType::StorageBuffer,
                    count: 1024,
                    stages: ShaderStageFlags::COMPUTE,
                    flags: DescriptorBindingFlags::PARTIALLY_BOUND | DescriptorBindingFlags::UPDATE_UNUSED_WHILE_PENDING,
                },
            ],
        })?;

        let per_frame_set_layout =
            ctx.create_descriptor_set_layout(DescriptorSetLayoutInfo {
                flags: DescriptorSetLayoutFlags::empty(),
                bindings: vec![
                    DescriptorSetLayoutBinding {
                        binding: 0,
                        count: 1,
                        ty: DescriptorType::StorageBuffer,
                        stages: ShaderStageFlags::COMPUTE,
                        flags: DescriptorBindingFlags::empty(),
                    },
                    DescriptorSetLayoutBinding {
                        binding: 1,
                        count: 1024,
                        ty: DescriptorType::StorageBuffer,
                        stages: ShaderStageFlags::COMPUTE,
                        flags: DescriptorBindingFlags::PARTIALLY_BOUND,
                    },
                ],
            })?;

        let layout = ctx.create_pipeline_layout(PipelineLayoutInfo {
            sets: vec![set_layout.clone(), per_frame_set_layout.clone()],
            push_constants: vec![PushConstant {
                stages: ShaderStageFlags::COMPUTE,
                offset: 0,
                size: std::mem::size_of::<InputOutMesh>() as u32,
            }],
        })?;

        let shader = ComputeShader::with_main(ctx.create_shader_module(
            Spirv::new(include_bytes!("pose/pose.comp.spv").to_vec()).into(),
        )?);

        let pipeline = ctx.create_compute_pipeline(ComputePipelineInfo {
            shader,
            layout: layout.clone(),
        })?;

        let set = ctx.create_descriptor_set(DescriptorSetInfo {
            layout: set_layout.clone(),
        })?;

        let per_frame_set0 = ctx.create_descriptor_set(DescriptorSetInfo {
            layout: per_frame_set_layout.clone(),
        })?;

        let per_frame_set1 = ctx.create_descriptor_set(DescriptorSetInfo {
            layout: per_frame_set_layout.clone(),
        })?;

        Ok(PosePass {
            layout,
            pipeline,
            set,
            per_frame_sets: [per_frame_set0, per_frame_set1],
            meshes: SparseDescriptors::new(),
            joints_buffer: None,
            joints_buffer_written: [false; 2],
        })
    }
}

impl Pass<'_> for PosePass {
    type Input = ();
    type Output = ();

    fn draw(
        &mut self,
        _: (),
        frame: u64,
        wait: &[(PipelineStageFlags, Semaphore)],
        signal: &[Semaphore],
        fence: Option<&Fence>,
        ctx: &mut Context,
        world: &mut World,
        bump: &Bump,
    ) -> Result<(), Report> {
        let findex = (frame & 1) as usize;
        let joints_descriptor;
        let mut pose_mesh_descriptors = BVec::new_in(bump);
        let mut writes = BVec::new_in(bump);

        let mut joints = BVec::new_in(bump);
        let mut to_dispatch = BVec::new_in(bump);

        for (_, (pose, mesh, renderable)) in world
            .query::<(&Pose, &PoseMesh, &Renderable)>()
            .with::<na::Isometry3<f32>>()
            .iter()
        {
            if pose.matrices().is_empty() {
                continue;
            }

            let joints_offset = joints.len() as u32;

            joints.extend_from_slice(pose.matrices());

            let vectors = mesh
                .bindings()
                .iter()
                .find(|binding| {
                    binding.layout == PositionNormalTangent3dUV::layout()
                })
                .unwrap();

            let vectors_buffer = vectors.buffer.clone();
            let vectors_offset = vectors.offset;
            let vectors_size: u64 = vectors.layout.stride as u64
                * renderable.mesh.vertex_count() as u64;

            assert_eq!(vectors_offset & 15, 0);

            pose_mesh_descriptors.push((
                vectors_buffer,
                vectors_offset,
                vectors_size,
            ));

            let (mesh_index, new) = self.meshes.index(renderable.mesh.clone());
            if new {
                let vectors = renderable
                    .mesh
                    .bindings()
                    .iter()
                    .find(|binding| {
                        binding.layout == PositionNormalTangent3dUV::layout()
                    })
                    .unwrap();

                let skin = renderable
                    .mesh
                    .bindings()
                    .iter()
                    .find(|binding| binding.layout == Skin::layout())
                    .unwrap();

                let vectors_buffer = vectors.buffer.clone();
                let vectors_offset = vectors.offset;
                let vectors_size: u64 = vectors.layout.stride as u64
                    * renderable.mesh.vertex_count() as u64;

                let skin_buffer = skin.buffer.clone();
                let skin_offset = skin.offset;
                let skin_size: u64 = skin.layout.stride as u64
                    * renderable.mesh.vertex_count() as u64;

                assert_eq!(vectors_offset & 15, 0);
                assert_eq!(skin_offset & 15, 0);

                // FIXME: Leak
                let vectors_desc = Descriptors::StorageBuffer(bump.alloc([(
                    vectors_buffer,
                    vectors_offset,
                    vectors_size,
                )]));

                let skin_desc = Descriptors::StorageBuffer(bump.alloc([(
                    skin_buffer,
                    skin_offset,
                    skin_size,
                )]));

                writes.push(WriteDescriptorSet {
                    set: &self.set,
                    binding: 0,
                    element: mesh_index,
                    descriptors: vectors_desc,
                });

                writes.push(WriteDescriptorSet {
                    set: &self.set,
                    binding: 1,
                    element: mesh_index,
                    descriptors: skin_desc,
                });
            }

            to_dispatch.push((
                mesh_index,
                joints_offset,
                renderable.mesh.vertex_count(),
            ));
        }

        if joints.is_empty() {
            assert!(to_dispatch.is_empty());
            return Ok(());
        }

        writes.push(WriteDescriptorSet {
            set: &self.per_frame_sets[findex],
            binding: 1,
            element: 0,
            descriptors: Descriptors::StorageBuffer(&pose_mesh_descriptors),
        });

        let joints_size = size_of_val_64(&joints[..])?;

        let joints_buffer = match &mut self.joints_buffer {
            Some(buffer) if buffer.info().size >= joints_size => {
                if !self.joints_buffer_written[findex] {
                    joints_descriptor =
                        [(buffer.share(), 0, buffer.info().size)];
                    writes.push(WriteDescriptorSet {
                        set: &self.per_frame_sets[findex],
                        binding: 0,
                        element: 0,
                        descriptors: Descriptors::StorageBuffer(
                            &joints_descriptor,
                        ),
                    });
                    self.joints_buffer_written[findex] = true;
                }
                buffer
            }
            _ => {
                let size = (joints_size + 4095) & !4095;
                let buffer = ctx.device.create_mappable_buffer(
                    BufferInfo {
                        size,
                        align: 255,
                        usage: BufferUsage::STORAGE,
                    },
                    MemoryUsage::UPLOAD | MemoryUsage::FAST_DEVICE_ACCESS,
                )?;

                joints_descriptor = [(buffer.share(), 0, size)];
                writes.push(WriteDescriptorSet {
                    set: &self.per_frame_sets[findex],
                    binding: 0,
                    element: 0,
                    descriptors: Descriptors::StorageBuffer(&joints_descriptor),
                });
                self.joints_buffer_written[findex] = true;

                self.joints_buffer = None;
                self.joints_buffer.get_or_insert(buffer)
            }
        };
        ctx.device.write_buffer(joints_buffer, 0, unsafe {
            std::mem::transmute::<&[_], &[u8]>(&joints[..])
        })?;
        ctx.device.update_descriptor_sets(&writes, &[]);

        let sets = [self.set.clone(), self.per_frame_sets[findex].clone()];

        let mut encoder = ctx.queue.create_encoder()?;

        encoder.bind_compute_pipeline(&self.pipeline);
        encoder.bind_compute_descriptor_sets(&self.layout, 0, &sets, &[]);

        for (index, &(mesh, joints_offset, vertex_count)) in
            to_dispatch.iter().enumerate()
        {
            encoder.push_constants(
                &self.layout,
                ShaderStageFlags::COMPUTE,
                0,
                bump.alloc([InputOutMesh {
                    joints_offset,
                    in_mesh: mesh,
                    out_mesh: index as u32,
                }]),
            );

            encoder.dispatch(vertex_count, 1, 1);
        }

        let cbuf = encoder.finish();
        ctx.queue.submit(wait, cbuf, signal, fence);

        Ok(())
    }
}

fn size_of_val_64<T: ?Sized>(val: &T) -> Result<u64, OutOfMemory> {
    size_of_val(val).try_into().map_err(|_| OutOfMemory)
}

#[derive(Clone, Copy)]
#[repr(C)]
struct InputOutMesh {
    joints_offset: u32,
    in_mesh: u32,
    out_mesh: u32,
}

unsafe impl Zeroable for InputOutMesh {}
unsafe impl Pod for InputOutMesh {}
