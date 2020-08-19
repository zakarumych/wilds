//!
//! Frame-pass that transforms vertices of mesh into specified pose.

use {
    super::Pass,
    crate::renderer::{Context, Mesh, PoseMesh},
    animate::skeletal3d::Pose,
    bumpalo::{Bump, collections::Vec as BVec},
    eyre::Report,
    hecs::World,
    illume::{
        Buffer, BufferUsage, Fence, MemoryUsageFlags, OutOfMemory,
        PipelineStageFlags, Semaphore, BufferInfo,
    },
    std::{convert::TryInto as _, mem::size_of_val},
    ultraviolet::{Isometry3, Mat4},
};

pub struct PosePass {
    transforms_buffer: Option<Buffer>,
}

impl Pass<'_> for PosePass {
    type Input = ();
    type Output = ();

    fn draw(
        &mut self,
        input: (),
        frame: u64,
        wait: &[(PipelineStageFlags, Semaphore)],
        signal: &[Semaphore],
        fence: Option<&Fence>,
        ctx: &mut Context,
        world: &mut World,
        bump: &Bump,
    ) -> Result<(), Report> {
        let mut matrices = BVec::new_in(bump);

        for (_, (mesh, pose_mesh, pose)) in
            world.query::<(&Mesh, &PoseMesh, &Pose)>().iter()
        {
            matrices.extend(
                pose.isometries()
                    .iter()
                    .map(|iso| iso.into_homogeneous_matrix()),
            );
        }

        let matrices_size = size_of_val_64(&matrices[..])?;

        let transforms_buffer = match &self.transforms_buffer {
            Some(buffer) if buffer.info().size >= matrices_size => buffer,
            _ => {
                let buffer = ctx.device.create_buffer(BufferInfo {
                    size: matrices_size,
                    align: 255,
                    usage: BufferUsage::STORAGE,
                    memory: MemoryUsageFlags::UPLOAD
                        | MemoryUsageFlags::FAST_DEVICE_ACCESS,
                })?;
                self.transforms_buffer = None;
                self.transforms_buffer.get_or_insert(buffer)
            }
        };

        ctx.device.write_memory(transforms_buffer, 0, &matrices[..]);

        Ok(())
    }
}

fn size_of_val_64<T: ?Sized>(val: &T) -> Result<u64, OutOfMemory> {
    size_of_val(val).try_into().map_err(|_| OutOfMemory)
}
