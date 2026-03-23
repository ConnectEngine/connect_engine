pub mod model_loader;

use bytemuck::{Pod, Zeroable};
pub use model_loader::*;
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
