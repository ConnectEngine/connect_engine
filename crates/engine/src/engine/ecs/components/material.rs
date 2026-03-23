use renderer::MaterialState;
use vulkanite::vk::DeviceAddress;

pub struct Material {
    pub ptr_data: DeviceAddress,
    pub state: MaterialState,
}
