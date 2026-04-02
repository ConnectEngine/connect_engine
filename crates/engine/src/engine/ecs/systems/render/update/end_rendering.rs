use bevy_ecs::system::{Res, ResMut};

use vulkan::vk::*;

use connect_renderer::*;

pub fn end_rendering_system(
    vulkan_context_resource: Res<VulkanContextResource>,
    renderer_context: Res<RendererContextResource>,
    textures_pool: ResMut<TexturesPoolResource>,
    frame_context: Res<FrameContextResource>,
) {
    let device = vulkan_context_resource.device.as_ref();
    let command_buffer = frame_context.command_buffer.unwrap();

    let swapchain_image = renderer_context.images[frame_context.swapchain_image_index as usize];

    let draw_image = textures_pool
        .get_image(frame_context.draw_texture_reference)
        .unwrap();

    let draw_image_extent3d = draw_image.extent;
    let draw_image_extent2d = Extent2D {
        width: draw_image_extent3d.width,
        height: draw_image_extent3d.height,
    };

    unsafe {
        device.cmd_end_rendering(command_buffer);
    }

    transition_image(
        device,
        command_buffer,
        draw_image.image,
        ImageLayout::GENERAL,
        ImageLayout::GENERAL,
        PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
        PipelineStageFlags2::BLIT,
        AccessFlags2::COLOR_ATTACHMENT_WRITE,
        AccessFlags2::TRANSFER_READ,
        draw_image.image_aspect_flags,
        frame_context
            .draw_texture_reference
            .texture_metadata
            .mip_levels_count,
    );

    transition_image(
        device,
        command_buffer,
        swapchain_image,
        ImageLayout::UNDEFINED,
        ImageLayout::GENERAL,
        PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
        PipelineStageFlags2::BLIT,
        AccessFlags2::NONE,
        AccessFlags2::TRANSFER_WRITE,
        ImageAspectFlags::COLOR,
        1,
    );

    copy_image_to_image(
        device,
        command_buffer,
        draw_image.image,
        swapchain_image,
        draw_image_extent2d,
        renderer_context.draw_extent,
    );

    transition_image(
        device,
        command_buffer,
        swapchain_image,
        ImageLayout::GENERAL,
        ImageLayout::PRESENT_SRC_KHR,
        PipelineStageFlags2::BLIT,
        PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
        AccessFlags2::TRANSFER_WRITE,
        AccessFlags2::NONE,
        ImageAspectFlags::COLOR,
        1,
    );

    unsafe {
        device.end_command_buffer(command_buffer).unwrap();
    }
}
