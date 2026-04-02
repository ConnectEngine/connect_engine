use connect_renderer::MaterialState;
use vulkan::vk::DeviceAddress;

pub struct Material {
    pub ptr_data: DeviceAddress,
    pub state: MaterialState,
}
