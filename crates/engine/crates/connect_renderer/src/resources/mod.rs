pub mod device_properties_resource;
pub mod frame_context_resource;
pub mod render_context_resource;
pub mod render_resources;
pub mod vulkan_context_resource;

pub use device_properties_resource::*;
pub use frame_context_resource::*;
pub use render_context_resource::*;
pub use render_resources::*;
pub use vulkan_context_resource::*;

use bytemuck::{Pod, Zeroable};
use padding_struct::padding_struct;
use vulkanite::vk::DeviceAddress;

#[repr(C)]
#[padding_struct]
#[derive(Default, Clone, Copy, Pod, Zeroable)]
pub struct MeshObject {
    pub device_address_vertex_buffer: DeviceAddress,
    pub device_address_vertex_indices_buffer: DeviceAddress,
    pub device_address_meshlets_buffer: DeviceAddress,
    pub device_address_local_indices_buffer: DeviceAddress,
}
