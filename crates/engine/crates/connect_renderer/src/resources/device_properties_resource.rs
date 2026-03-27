use bevy_ecs::resource::Resource;
use vulkanite::vk::PhysicalDeviceDescriptorBufferPropertiesEXT;

#[derive(Resource)]
pub struct DevicePropertiesResource {
    pub descriptor_buffer_properties: PhysicalDeviceDescriptorBufferPropertiesEXT<'static>,
}
