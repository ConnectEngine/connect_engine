use bevy_ecs::system::{Res, ResMut};

use crate::engine::resources::FrameContextResource;
use vulkanite::vk::*;

use connect_renderer::*;

pub fn end_rendering_system(
    renderer_context: Res<RendererContextResource>,
    textures_pool: ResMut<TexturesPoolResource>,
    frame_context: Res<FrameContextResource>,
) {
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

    command_buffer.end_rendering();

    transition_image(
        command_buffer,
        draw_image.image,
        ImageLayout::General,
        ImageLayout::General,
        PipelineStageFlags2::ColorAttachmentOutput,
        PipelineStageFlags2::Blit,
        AccessFlags2::ColorAttachmentWrite,
        AccessFlags2::TransferRead,
        draw_image.image_aspect_flags,
        frame_context
            .draw_texture_reference
            .texture_metadata
            .mip_levels_count,
    );

    transition_image(
        command_buffer,
        swapchain_image,
        ImageLayout::Undefined,
        ImageLayout::General,
        PipelineStageFlags2::ColorAttachmentOutput,
        PipelineStageFlags2::Blit,
        AccessFlags2::None,
        AccessFlags2::TransferWrite,
        ImageAspectFlags::Color,
        1,
    );

    copy_image_to_image(
        command_buffer,
        draw_image.image,
        swapchain_image,
        draw_image_extent2d,
        renderer_context.draw_extent,
    );

    transition_image(
        command_buffer,
        swapchain_image,
        ImageLayout::General,
        ImageLayout::PresentSrcKHR,
        PipelineStageFlags2::Blit,
        PipelineStageFlags2::ColorAttachmentOutput,
        AccessFlags2::TransferWrite,
        AccessFlags2::None,
        ImageAspectFlags::Color,
        1,
    );

    command_buffer.end().unwrap();
}
