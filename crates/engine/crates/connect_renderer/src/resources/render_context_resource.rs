use bevy_ecs::resource::Resource;
use vulkan::vk::*;

use crate::{CommandGroup, TextureReference};

pub struct FrameData {
    pub command_group: CommandGroup,
    pub swapchain_semaphore: Semaphore,
    pub render_semaphore: Semaphore,
    pub draw_texture_reference: TextureReference,
    pub depth_texture_reference: TextureReference,
}

#[derive(Clone, Copy)]
pub struct UploadContext {
    pub command_group: CommandGroup,
}

#[derive(Resource)]
pub struct RendererContextResource {
    pub images: Vec<Image>,
    pub image_views: Vec<ImageView>,
    pub frame_overlap: usize,
    pub frames_data: Vec<FrameData>,
    pub upload_context: UploadContext,
    pub frame_number: usize,
    pub draw_extent: Extent2D,
}

impl RendererContextResource {
    pub fn get_current_frame_data(&self) -> &FrameData {
        &self.frames_data[self.frame_number % self.frame_overlap]
    }
}
