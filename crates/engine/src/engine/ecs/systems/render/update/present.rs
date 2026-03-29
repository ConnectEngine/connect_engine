use bevy_ecs::system::{Res, ResMut};
use vulkanite::vk::*;

use connect_renderer::*;

pub fn present_system(
    vulkan_ctx: Res<VulkanContextResource>,
    mut render_ctx: ResMut<RendererContextResource>,
    frame_ctx: Res<FrameContextResource>,
) {
    let _device = &vulkan_ctx.device;
    let frame_data = render_ctx.get_current_frame_data();
    let command_buffer = frame_data.command_group.command_buffer;
    let swapchain_image_index = frame_ctx.swapchain_image_index;

    let command_buffer_submit_infos = [command_buffer_submit_info(&command_buffer)];

    let wait_semaphore_submit_infos = [semaphore_submit_info(
        PipelineStageFlags2::ColorAttachmentOutput,
        &frame_data.swapchain_semaphore,
    )];
    let signal_semaphore_submit_infos = [semaphore_submit_info(
        PipelineStageFlags2::AllGraphics,
        &frame_data.render_semaphore,
    )];

    let submit_info = submit_info(
        &command_buffer_submit_infos,
        &wait_semaphore_submit_infos,
        &signal_semaphore_submit_infos,
    );

    let submit_infos = [submit_info];
    vulkan_ctx
        .graphics_queue
        .submit2(&submit_infos, Some(frame_data.command_group.fence))
        .unwrap();

    let swapchains = [vulkan_ctx.swapchain];
    let wait_semaphores = [frame_data.render_semaphore];
    let image_indicies = [swapchain_image_index];

    let present_info = PresentInfoKHR::default()
        .swapchain(swapchains.as_slice(), &image_indicies, None::<()>)
        .wait_semaphores(wait_semaphores.as_slice());

    vulkan_ctx
        .graphics_queue
        .present_khr(&present_info)
        .unwrap();

    render_ctx.frame_number += 1;
}
