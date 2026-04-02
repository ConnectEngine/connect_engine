use bevy_ecs::system::{Res, ResMut};
use vulkan::vk::PipelineStageFlags2;
use vulkan::vk::*;

use connect_renderer::*;

pub fn present_system(
    vulkan_ctx: Res<VulkanContextResource>,
    mut render_ctx: ResMut<RendererContextResource>,
    frame_ctx: Res<FrameContextResource>,
) {
    let device = vulkan_ctx.device.as_ref();
    let frame_data = render_ctx.get_current_frame_data();
    let command_buffer = frame_data.command_group.command_buffer;
    let swapchain_image_index = frame_ctx.swapchain_image_index;

    let command_buffer_submit_infos = [command_buffer_submit_info(command_buffer)];

    let wait_semaphore_submit_infos = [semaphore_submit_info(
        PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
        frame_data.swapchain_semaphore,
    )];
    let signal_semaphore_submit_infos = [semaphore_submit_info(
        PipelineStageFlags2::ALL_GRAPHICS,
        frame_data.render_semaphore,
    )];

    let submit_info = submit_info(
        &command_buffer_submit_infos,
        &wait_semaphore_submit_infos,
        &signal_semaphore_submit_infos,
    );

    let submit_infos = [submit_info];
    unsafe {
        device
            .queue_submit2(
                vulkan_ctx.graphics_queue,
                &submit_infos,
                frame_data.command_group.fence,
            )
            .unwrap();
    }

    let swapchains = [vulkan_ctx.swapchain];
    let wait_semaphores = [frame_data.render_semaphore];
    let image_indicies = [swapchain_image_index];

    let present_info = PresentInfoKHRBuilder::default()
        .swapchains(swapchains.as_slice())
        .image_indices(&image_indicies)
        .wait_semaphores(wait_semaphores.as_slice());

    unsafe {
        device
            .queue_present_khr(vulkan_ctx.graphics_queue, &present_info)
            .unwrap();
    }

    render_ctx.frame_number += 1;
}
