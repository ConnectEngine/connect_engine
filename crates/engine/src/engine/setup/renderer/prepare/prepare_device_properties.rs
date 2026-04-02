use bevy_ecs::world::World;
use vulkan::vk::*;

use crate::engine::Engine;

use connect_renderer::*;

impl Engine {
    pub(crate) fn create_device_properties(world: &World) -> DevicePropertiesResource {
        let vulkan_context_resource = world.get_resource_ref::<VulkanContextResource>().unwrap();

        let mut descriptor_buffer_properties =
            PhysicalDeviceDescriptorBufferPropertiesEXT::default();
        let mut physical_device_properties = PhysicalDeviceProperties2Builder::default()
            .push_next(&mut descriptor_buffer_properties)
            .build();

        unsafe {
            vulkan_context_resource
                .instance
                .get_physical_device_properties2(
                    vulkan_context_resource.physical_device,
                    &mut physical_device_properties,
                );
        }
        println!("YES");

        DevicePropertiesResource {
            descriptor_buffer_properties: descriptor_buffer_properties,
        }
    }
}
