use bevy_ecs::world::World;
use vulkanite::vk::{rs::*, *};
use winit::window::Window;

use crate::engine::Engine;

use connect_renderer::*;

impl Engine {
    pub(crate) fn create_renderer_context(
        window: &dyn Window,
        world: &World,
    ) -> RendererContextResource {
        let vulkan_context_resource = world.get_resource_ref::<VulkanContextResource>().unwrap();
        let device = vulkan_context_resource.device;
        let swapchain = &vulkan_context_resource.swapchain;

        let images: Vec<Image> = device.get_swapchain_images_khr(*swapchain).unwrap();
        let image_views: Vec<ImageView> = images
            .iter()
            .map(|img| {
                device
                    .create_image_view(
                        &ImageViewCreateInfo::default()
                            .image(img)
                            .view_type(ImageViewType::Type2D)
                            .format(vulkan_context_resource.surface_format.format)
                            .subresource_range(ImageSubresourceRange {
                                aspect_mask: ImageAspectFlags::Color,
                                base_mip_level: 0,
                                level_count: 1,
                                base_array_layer: 0,
                                layer_count: 1,
                            }),
                    )
                    .unwrap()
            })
            .collect();
        let frame_overlap = image_views.len();

        let command_pool_info = CommandPoolCreateInfo::default()
            .flags(CommandPoolCreateFlags::ResetCommandBuffer)
            .queue_family_index(vulkan_context_resource.queue_family_index as _);

        let device = &vulkan_context_resource.device;
        let frames_data = (0..frame_overlap)
            .map(|_| {
                let command_pool = device.create_command_pool(&command_pool_info).unwrap();

                let command_buffer_allocate_info = CommandBufferAllocateInfo::default()
                    .command_pool(&command_pool)
                    .level(vulkanite::vk::CommandBufferLevel::Primary)
                    .command_buffer_count(1);

                let command_buffers: Vec<CommandBuffer> = device
                    .allocate_command_buffers(&command_buffer_allocate_info)
                    .unwrap();
                let command_buffer = command_buffers[0];

                let fence_info = FenceCreateInfo::default().flags(FenceCreateFlags::Signaled);
                let render_fence = device.create_fence(&fence_info).unwrap();

                let semaphore_create_info = SemaphoreCreateInfo::default();
                let swapchain_semaphore = device.create_semaphore(&semaphore_create_info).unwrap();
                let render_semaphore = device.create_semaphore(&semaphore_create_info).unwrap();

                let command_group = CommandGroup {
                    command_pool,
                    command_buffer,
                    fence: render_fence,
                };
                FrameData {
                    command_group,
                    swapchain_semaphore,
                    render_semaphore,
                    draw_texture_reference: Default::default(),
                    depth_texture_reference: Default::default(),
                }
            })
            .collect();

        let surface_size = window.surface_size();
        let draw_extent = Extent2D {
            width: surface_size.width,
            height: surface_size.height,
        };

        let fence_info = FenceCreateInfo::default();
        let fence = device.create_fence(&fence_info).unwrap();

        let command_pool = device.create_command_pool(&command_pool_info).unwrap();

        let command_buffer_allocate_info = CommandBufferAllocateInfo::default()
            .command_pool(&command_pool)
            .level(vulkanite::vk::CommandBufferLevel::Primary)
            .command_buffer_count(1);

        let command_buffers: Vec<CommandBuffer> = device
            .allocate_command_buffers(&command_buffer_allocate_info)
            .unwrap();
        let command_buffer = command_buffers[0];

        let upload_context = UploadContext {
            command_group: CommandGroup {
                command_pool,
                command_buffer,
                fence,
            },
        };

        RendererContextResource {
            images,
            image_views,
            frame_overlap,
            draw_extent,
            frames_data,
            frame_number: Default::default(),
            upload_context,
        }
    }
}
