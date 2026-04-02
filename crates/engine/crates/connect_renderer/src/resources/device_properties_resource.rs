use bevy_ecs::resource::Resource;
use vulkan::vk::PhysicalDeviceDescriptorBufferPropertiesEXT;

#[derive(Resource)]
pub struct DevicePropertiesResource {
    pub descriptor_buffer_properties: PhysicalDeviceDescriptorBufferPropertiesEXT,
}
