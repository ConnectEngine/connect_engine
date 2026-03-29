use bevy_ecs::system::{Res, ResMut};
use connect_renderer::*;
use vulkanite::vk::*;

pub fn prepare_frame_system(
    vulkan_ctx: Res<VulkanContextResource>,
    render_ctx: Res<RendererContextResource>,
    mut renderer_resources: ResMut<RendererResources>,
    mut frame_ctx: ResMut<FrameContextResource>,
) {
    let device = &vulkan_ctx.device;
    let frame_data = render_ctx.get_current_frame_data();
    let fences = [frame_data.command_group.fence];

    device
        .wait_for_fences(fences.as_slice(), true, u64::MAX)
        .unwrap();
    device.reset_fences(fences.as_slice()).unwrap();

    let (_status, swapchain_image_index) = device
        .acquire_next_image_khr(
            vulkan_ctx.swapchain,
            u64::MAX,
            Some(frame_data.swapchain_semaphore),
            Default::default(),
        )
        .unwrap();
    frame_ctx.swapchain_image_index = swapchain_image_index;

    let instances_objects_buffer = renderer_resources
        .resources_pool
        .instances_buffer
        .as_mut()
        .unwrap();
    instances_objects_buffer.next_buffer();

    let scene_data_buffer = renderer_resources
        .resources_pool
        .scene_data_buffer
        .as_mut()
        .unwrap();
    scene_data_buffer.next_buffer();

    frame_data
        .command_group
        .command_buffer
        .reset(CommandBufferResetFlags::ReleaseResources)
        .unwrap();
}
