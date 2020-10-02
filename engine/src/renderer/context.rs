use {
    bumpalo::{collections::Vec as BVec, Bump},
    bytemuck::Pod,
    eyre::Report,
    illume::{
        arith_ge, Buffer, BufferCopy, BufferImageCopy, BufferInfo, BufferUsage,
        CreateImageError, Device, Extent3d, Image, ImageInfo,
        ImageMemoryBarrier, ImageSubresourceLayers, ImageSubresourceRange,
        ImageUsage, Layout, MappingError, MemoryUsageFlags, Offset3d,
        OutOfMemory, PipelineStageFlags, Queue,
    },
    std::{convert::TryFrom as _, ops::Deref},
};

pub struct Context {
    pub device: Device,
    pub queue: Queue,
    buffer_uploads: Vec<BufferUpload>,
    image_uploads: Vec<ImageUpload>,
}

struct BufferUpload {
    staging: Buffer,
    buffer: Buffer,
    offset: u64,
}

struct ImageUpload {
    staging: Buffer,
    image: Image,
    layout: Option<Layout>,
    row_length: u32,
    image_height: u32,
    subresource: ImageSubresourceLayers,
    offset: Offset3d,
    extent: Extent3d,
}

impl Context {
    pub fn new(device: Device, queue: Queue) -> Self {
        Context {
            device,
            queue,
            buffer_uploads: Vec::new(),
            image_uploads: Vec::new(),
        }
    }

    pub fn upload_buffer<T>(
        &mut self,
        buffer: &Buffer,
        offset: u64,
        data: &[T],
    ) -> Result<(), MappingError>
    where
        T: Pod,
    {
        if buffer.info().memory.intersects(
            MemoryUsageFlags::HOST_ACCESS
                | MemoryUsageFlags::UPLOAD
                | MemoryUsageFlags::DOWNLOAD,
        ) {
            self.device.write_memory(buffer, offset, data)?;
            Ok(())
        } else {
            let staging = self.device.create_buffer_static(
                BufferInfo {
                    align: 15,
                    size: u64::try_from(data.len()).map_err(|_| OutOfMemory)?,
                    usage: BufferUsage::TRANSFER_SRC,
                    memory: MemoryUsageFlags::UPLOAD,
                },
                data,
            )?;

            self.buffer_uploads.push(BufferUpload {
                staging,
                buffer: buffer.clone(),
                offset,
            });

            Ok(())
        }
    }

    pub fn upload_image<T>(
        &mut self,
        image: &Image,
        layout: Option<Layout>,
        row_length: u32,
        image_height: u32,
        subresource: ImageSubresourceLayers,
        offset: Offset3d,
        extent: Extent3d,
        data: &[T],
    ) -> Result<(), OutOfMemory>
    where
        T: Pod,
    {
        let staging = self.device.create_buffer_static(
            BufferInfo {
                align: 15,
                size: u64::try_from(data.len()).map_err(|_| OutOfMemory)?,
                usage: BufferUsage::TRANSFER_SRC,
                memory: MemoryUsageFlags::UPLOAD,
            },
            data,
        )?;

        self.image_uploads.push(ImageUpload {
            staging,
            image: image.clone(),
            layout,
            row_length,
            image_height,
            subresource,
            offset,
            extent,
        });

        Ok(())
    }

    pub fn create_buffer_static<T>(
        &mut self,
        mut info: BufferInfo,
        data: &[T],
    ) -> Result<Buffer, OutOfMemory>
    where
        T: Pod,
    {
        assert!(arith_ge(info.size, data.len()));
        if info.memory.intersects(
            MemoryUsageFlags::HOST_ACCESS
                | MemoryUsageFlags::UPLOAD
                | MemoryUsageFlags::DOWNLOAD,
        ) {
            self.device.create_buffer_static(info, data)
        } else {
            info.usage |= BufferUsage::TRANSFER_DST;
            let buffer = self.device.create_buffer(info)?;
            match self.upload_buffer(&buffer, 0, data) {
                Ok(()) => {}
                Err(MappingError::OutOfMemory { .. }) => {
                    return Err(OutOfMemory)
                }
                _ => unreachable!(),
            }
            Ok(buffer)
        }
    }

    pub fn create_image_static<T>(
        &mut self,
        mut info: ImageInfo,
        row_length: u32,
        image_height: u32,
        data: &[T],
    ) -> Result<Image, CreateImageError>
    where
        T: Pod,
    {
        info.usage |= ImageUsage::TRANSFER_DST;
        let subresource = ImageSubresourceLayers::all_layers(&info, 0);
        let image = self.device.create_image(info)?;
        self.upload_image(
            &image,
            None,
            row_length,
            image_height,
            subresource,
            Offset3d::ZERO,
            info.extent.into_3d(),
            data,
        )?;
        Ok(image)
    }

    pub fn flush_uploads(&mut self, bump: &Bump) -> Result<(), Report> {
        if self.buffer_uploads.is_empty() && self.image_uploads.is_empty() {
            return Ok(());
        }

        let mut encoder = self.queue.create_encoder()?;

        if !self.buffer_uploads.is_empty() {
            tracing::debug!("Uploading buffers");

            for upload in &self.buffer_uploads {
                encoder.copy_buffer(
                    &upload.staging,
                    &upload.buffer,
                    bump.alloc([BufferCopy {
                        src_offset: 0,
                        dst_offset: upload.offset,
                        size: upload.staging.info().size,
                    }]),
                )
            }
        }

        if !self.image_uploads.is_empty() {
            tracing::debug!("Uploading images");

            let mut images =
                BVec::with_capacity_in(self.image_uploads.len(), bump);

            for upload in &self.image_uploads {
                let switch_layout = match upload.layout {
                    Some(Layout::General)
                    | Some(Layout::TransferDstOptimal) => false,
                    _ => true,
                };

                if switch_layout {
                    images.push(ImageMemoryBarrier {
                        image: bump.alloc(upload.image.clone()),
                        old_layout: None,
                        new_layout: Layout::TransferDstOptimal,
                        family_transfer: None,
                        subresource: ImageSubresourceRange::whole(
                            upload.image.info(),
                        ),
                    });
                }
            }

            let images_len = images.len();

            encoder.image_barriers(
                PipelineStageFlags::TOP_OF_PIPE,
                PipelineStageFlags::TRANSFER,
                images.into_bump_slice(),
            );

            for upload in &self.image_uploads {
                encoder.copy_buffer_to_image(
                    &upload.staging,
                    &upload.image,
                    if upload.layout == Some(Layout::General) {
                        Layout::General
                    } else {
                        Layout::TransferDstOptimal
                    },
                    bump.alloc([BufferImageCopy {
                        buffer_offset: 0,
                        buffer_row_length: upload.row_length,
                        buffer_image_height: upload.image_height,
                        image_subresource: upload.subresource,
                        image_offset: upload.offset,
                        image_extent: upload.extent,
                    }]),
                )
            }

            let mut images = BVec::with_capacity_in(images_len, bump);

            for upload in &self.image_uploads {
                let switch_layout = match upload.layout {
                    Some(Layout::General)
                    | Some(Layout::TransferDstOptimal) => false,
                    _ => true,
                };

                if switch_layout {
                    images.push(ImageMemoryBarrier {
                        image: bump.alloc(upload.image.clone()),
                        old_layout: Some(Layout::TransferDstOptimal),
                        new_layout: upload.layout.unwrap_or(Layout::General),
                        family_transfer: None,
                        subresource: ImageSubresourceRange::whole(
                            upload.image.info(),
                        ),
                    });
                }
            }

            encoder.image_barriers(
                PipelineStageFlags::TRANSFER,
                PipelineStageFlags::TOP_OF_PIPE,
                images.into_bump_slice(),
            );
        }

        self.queue.submit_no_semaphores(encoder.finish(), None);

        self.buffer_uploads.clear();
        self.image_uploads.clear();
        Ok(())
    }
}

impl Deref for Context {
    type Target = Device;

    fn deref(&self) -> &Device {
        &self.device
    }
}
