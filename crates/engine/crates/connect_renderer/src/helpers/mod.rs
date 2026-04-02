use vulkan::{Device, vk::*};

#[derive(Clone, Copy)]
pub struct ShaderInfo<'a> {
    pub path: &'a str,
    pub flags: ShaderCreateFlagsEXT,
    pub stage: ShaderStageFlags,
    pub next_stage: ShaderStageFlags,
    pub descriptor_layouts: &'a [DescriptorSetLayout],
    pub push_constant_ranges: Option<&'a [PushConstantRange]>,
}

pub fn create_command_buffer_begin_info<'a>(
    flags: CommandBufferUsageFlags,
) -> CommandBufferBeginInfo {
    CommandBufferBeginInfoBuilder::default()
        .flags(flags)
        .build()
}

pub fn transition_image(
    device: &vulkan::Device,
    command_buffer: CommandBuffer,
    image: Image,
    old_image_layout: ImageLayout,
    new_image_layout: ImageLayout,
    src_stage_mask: PipelineStageFlags2,
    dst_stage_mask: PipelineStageFlags2,
    src_access_mask: AccessFlags2,
    dst_access_mask: AccessFlags2,
    image_aspect_flags: ImageAspectFlags,
    mip_levels_count: u32,
) {
    let image_memory_barrier_builder = ImageMemoryBarrier2Builder::default()
        .src_stage_mask(src_stage_mask)
        .src_access_mask(src_access_mask)
        .dst_stage_mask(dst_stage_mask)
        .dst_access_mask(dst_access_mask)
        .old_layout(old_image_layout)
        .new_layout(new_image_layout)
        .subresource_range(image_subresource_range(
            image_aspect_flags,
            mip_levels_count,
        ));

    let image_memory_barrier = image_memory_barrier_builder.image(image).build();

    let image_memory_barriers = [image_memory_barrier];
    let dependency_info = DependencyInfoBuilder::default()
        .image_memory_barriers(&image_memory_barriers)
        .build();

    unsafe {
        device.cmd_pipeline_barrier2(command_buffer, &dependency_info);
    }
}

pub fn image_subresource_range(
    aspect_mask: ImageAspectFlags,
    mip_levels_count: u32,
) -> ImageSubresourceRange {
    ImageSubresourceRange {
        aspect_mask,
        base_mip_level: Default::default(),
        level_count: mip_levels_count,
        base_array_layer: Default::default(),
        layer_count: REMAINING_ARRAY_LAYERS,
    }
}

pub fn semaphore_submit_info<'a>(
    stage_mask: PipelineStageFlags2,
    semaphore: Semaphore,
) -> SemaphoreSubmitInfo {
    SemaphoreSubmitInfoBuilder::default()
        .semaphore(semaphore)
        .stage_mask(stage_mask)
        .build()
}

pub fn command_buffer_submit_info<'a>(command_buffer: CommandBuffer) -> CommandBufferSubmitInfo {
    CommandBufferSubmitInfoBuilder::default()
        .command_buffer(command_buffer)
        .build()
}

pub fn submit_info<'a>(
    command_buffer_submit_infos: &'a [CommandBufferSubmitInfo],
    wait_semaphores: &'a [SemaphoreSubmitInfo],
    signal_semaphores: &'a [SemaphoreSubmitInfo],
) -> SubmitInfo2 {
    SubmitInfo2Builder::default()
        .wait_semaphore_infos(wait_semaphores)
        .signal_semaphore_infos(signal_semaphores)
        .command_buffer_infos(command_buffer_submit_infos)
        .build()
}

pub fn copy_image_to_image(
    device: &Device,
    command_buffer: CommandBuffer,
    source_image: Image,
    destination_image: Image,
    src_extent: Extent2D,
    dst_extent: Extent2D,
) {
    let src_offsets = [
        Offset3D::default(),
        Offset3D {
            x: src_extent.width as _,
            y: src_extent.height as _,
            z: 1,
        },
    ];
    let dst_offsets = [
        Offset3D::default(),
        Offset3D {
            x: dst_extent.width as _,
            y: dst_extent.height as _,
            z: 1,
        },
    ];

    let src_subresource = ImageSubresourceLayers {
        aspect_mask: ImageAspectFlags::COLOR,
        mip_level: Default::default(),
        base_array_layer: Default::default(),
        layer_count: 1,
    };
    let dst_subresource = ImageSubresourceLayers {
        aspect_mask: ImageAspectFlags::COLOR,
        mip_level: Default::default(),
        base_array_layer: Default::default(),
        layer_count: 1,
    };
    let blit_region = ImageBlit2Builder::default()
        .src_subresource(src_subresource)
        .src_offsets(src_offsets)
        .dst_subresource(dst_subresource)
        .dst_offsets(dst_offsets)
        .build();

    let regions = [blit_region];
    let image_blit_info = BlitImageInfo2Builder::default()
        .src_image_layout(ImageLayout::GENERAL)
        .dst_image_layout(ImageLayout::GENERAL)
        .filter(Filter::LINEAR)
        .src_image(source_image)
        .dst_image(destination_image)
        .regions(&regions)
        .build();

    unsafe {
        device.cmd_blit_image2(command_buffer, &image_blit_info);
    }
}
