use bevy_ecs::resource::Resource;
use vma::Allocator;
use vulkanite::vk::{
    AccessFlags2, BufferImageCopy, CommandBufferBeginInfo, CommandBufferUsageFlags,
    CommandPoolResetFlags, Extent3D, ImageLayout, ImageSubresourceLayers, PipelineStageFlags2,
    SubmitInfo, SurfaceFormatKHR,
    rs::{
        DebugUtilsMessengerEXT, Device, Instance, PhysicalDevice, Queue, SurfaceKHR, SwapchainKHR,
    },
};

use crate::{AllocatedImage, BuffersPoolResource, UploadContext, transition_image};

#[derive(Resource)]
pub struct VulkanContextResource {
    pub instance: Instance,
    pub debug_utils_messenger: Option<DebugUtilsMessengerEXT>,
    pub surface: SurfaceKHR,
    pub device: Device,
    pub physical_device: PhysicalDevice,
    pub allocator: Allocator,
    pub graphics_queue: Queue,
    pub transfer_queue: Queue,
    pub queue_family_index: usize,
    pub swapchain: SwapchainKHR,
    pub surface_format: SurfaceFormatKHR,
}

impl VulkanContextResource {
    pub fn transfer_data_to_image(
        &self,
        allocated_image: &AllocatedImage,
        buffers_pool: &mut BuffersPoolResource,
        data_to_copy: *const std::ffi::c_void,
        upload_context: &UploadContext,
        size: Option<usize>,
    ) {
        let texture_metadata = allocated_image.texture_metadata;
        let command_buffer = upload_context.command_group.command_buffer;

        let command_buffer_begin_info = CommandBufferBeginInfo {
            flags: CommandBufferUsageFlags::OneTimeSubmit,
            ..Default::default()
        };

        command_buffer.begin(&command_buffer_begin_info).unwrap();

        let size = match size {
            Some(size) => size,
            None => (texture_metadata.width * texture_metadata.height * 8) as usize,
        };

        let staging_buffer_reference =
            unsafe { &*(&buffers_pool.get_staging_buffer_reference() as *const _) };
        unsafe {
            buffers_pool.transfer_data_to_buffer_raw(
                *staging_buffer_reference,
                data_to_copy,
                size as _,
            );
        }

        transition_image(
            command_buffer,
            allocated_image.image,
            ImageLayout::Undefined,
            ImageLayout::General,
            PipelineStageFlags2::None,
            PipelineStageFlags2::Copy,
            AccessFlags2::None,
            AccessFlags2::TransferWrite,
            allocated_image.subresource_range.aspect_mask,
            texture_metadata.mip_levels_count,
        );

        let mut current_buffer_offset = 0;

        let mut mip_width = texture_metadata.width;
        let mut mip_height = texture_metadata.height;
        let mut mip_depth = 1;

        let mut buffer_image_copies = Vec::with_capacity(texture_metadata.mip_levels_count as _);
        for mip_map_level_index in 0..texture_metadata.mip_levels_count {
            let buffer_image_copy = BufferImageCopy {
                buffer_offset: current_buffer_offset,
                image_subresource: ImageSubresourceLayers {
                    aspect_mask: allocated_image.subresource_range.aspect_mask,
                    mip_level: mip_map_level_index,
                    base_array_layer: Default::default(),
                    layer_count: 1,
                },
                image_extent: Extent3D {
                    width: mip_width,
                    height: mip_height,
                    depth: mip_depth,
                },
                ..Default::default()
            };
            let blocks_wide = mip_width.div_ceil(4);
            let blocks_high = mip_height.div_ceil(4);

            let block_size_in_bytes = 8;

            let current_mip_size =
                (blocks_wide * blocks_high) as u64 * block_size_in_bytes * mip_depth as u64;

            current_buffer_offset += current_mip_size;

            mip_width = (mip_width / 2).max(1);
            mip_height = (mip_height / 2).max(1);
            mip_depth = (mip_depth / 2).max(1);

            buffer_image_copies.push(buffer_image_copy);
        }

        upload_context
            .command_group
            .command_buffer
            .copy_buffer_to_image(
                buffers_pool
                    .get_buffer(*staging_buffer_reference)
                    .unwrap()
                    .buffer,
                allocated_image.image,
                ImageLayout::General,
                &buffer_image_copies,
            );

        transition_image(
            command_buffer,
            allocated_image.image,
            ImageLayout::General,
            ImageLayout::General,
            PipelineStageFlags2::Copy,
            PipelineStageFlags2::FragmentShader,
            AccessFlags2::TransferWrite,
            AccessFlags2::ShaderSampledRead,
            allocated_image.subresource_range.aspect_mask,
            texture_metadata.mip_levels_count,
        );

        command_buffer.end().unwrap();

        let command_buffers = [command_buffer];
        let queue_submits = [SubmitInfo::default().command_buffers(command_buffers.as_slice())];

        self.transfer_queue
            .submit(&queue_submits, Some(upload_context.command_group.fence))
            .unwrap();

        let fences_to_wait = [upload_context.command_group.fence];
        self.device
            .wait_for_fences(fences_to_wait.as_slice(), true, u64::MAX)
            .unwrap();
        self.device.reset_fences(fences_to_wait.as_slice()).unwrap();

        self.device
            .reset_command_pool(
                upload_context.command_group.command_pool,
                CommandPoolResetFlags::ReleaseResources,
            )
            .unwrap();
    }
}
