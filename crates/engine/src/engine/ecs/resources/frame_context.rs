use bevy_ecs::resource::Resource;
use math::Mat4;
use renderer::TextureReference;
use vulkanite::vk::rs::CommandBuffer;

#[derive(Default, Resource)]
pub struct FrameContext {
    pub swapchain_image_index: u32,
    pub command_buffer: Option<CommandBuffer>,
    pub draw_texture_reference: TextureReference,
    pub depth_texture_reference: TextureReference,
    pub world_matrix: Mat4,
}
