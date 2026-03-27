use bevy_ecs::world::World;
use vulkanite::vk::*;

use crate::engine::Engine;

use connect_renderer::*;

impl Engine {
    pub(crate) fn create_device_properties(world: &World) -> DevicePropertiesResource {
        let vulkan_context_resource = world.get_resource_ref::<VulkanContextResource>().unwrap();

        let (_, _, descriptor_buffer_properties): (
            _,
            PhysicalDeviceVulkan11Properties,
            PhysicalDeviceDescriptorBufferPropertiesEXT,
        ) = vulkan_context_resource.physical_device.get_properties2();

        DevicePropertiesResource {
            descriptor_buffer_properties,
        }
    }
}
