use bevy_ecs::resource::Resource;
use connect_math::*;
use vulkanite::vk::rs::CommandBuffer;

use crate::TextureReference;

#[derive(Default, Resource)]
pub struct FrameContextResource {
    pub swapchain_image_index: u32,
    pub command_buffer: Option<CommandBuffer>,
    pub draw_texture_reference: TextureReference,
    pub depth_texture_reference: TextureReference,
    pub world_matrix: Mat4,
}
