use std::sync::Arc;

use bevy_ecs::world::World;
use connect_renderer::*;
use vulkan::{Device, vk::*};

use crate::engine::{Engine, ecs::audio::Audio};

impl Engine {
    pub fn prepare_renderer_resources(world: &mut World) {
        let vulkan_context = world.get_resource_ref::<VulkanContextResource>().unwrap();
        let render_context = world.get_resource_ref::<RendererContextResource>().unwrap();
        let device_properties_resource = world
            .get_resource_ref::<DevicePropertiesResource>()
            .unwrap();

        let resources_pool = ResourcesPool::new();

        let upload_command_group = render_context.upload_context.command_group;

        let device = vulkan_context.device.clone();
        let allocator = vulkan_context.allocator.clone();

        let renderer_resources = RendererResources {
            default_texture_reference: Default::default(),
            fallback_texture_reference: Default::default(),
            default_sampler_reference: Default::default(),
            mesh_objects_buffer_reference: Default::default(),
            gradient_compute_shader_object: Default::default(),
            task_shader_object: Default::default(),
            mesh_shader_object: Default::default(),
            fragment_shader_object: Default::default(),
            resources_pool,
            is_printed_scene_hierarchy: true,
            materials_data_buffer_reference: Default::default(),
        };

        let mut buffers_pool = BuffersPoolResource::new(
            vulkan_context.instance.clone(),
            device.clone(),
            allocator.clone(),
            upload_command_group,
            vulkan_context.transfer_queue,
        );
        let textures_pool = TexturesPoolResource::new(device.clone(), allocator.clone());
        let samplers_pool = SamplersPool::new(device.clone());
        let mesh_buffers_pool = MeshBuffersPool::new(Default::default(), 5_120);

        let push_constant_range = PushConstantRange {
            stage_flags: ShaderStageFlags::MESH_EXT
                | ShaderStageFlags::FRAGMENT
                | ShaderStageFlags::COMPUTE
                | ShaderStageFlags::TASK_EXT,
            offset: Default::default(),
            size: std::mem::size_of::<GraphicsPushConstant>() as _,
        };

        let push_constant_ranges = [push_constant_range];
        let descriptor_set_handle = Self::create_descriptor_set_handle(
            device,
            &mut buffers_pool,
            &device_properties_resource,
            &push_constant_ranges,
        );

        let audio = Audio::new();

        world.insert_resource(renderer_resources);
        world.insert_resource(descriptor_set_handle);
        world.insert_resource(buffers_pool);
        world.insert_resource(samplers_pool);
        world.insert_resource(textures_pool);
        world.insert_resource(mesh_buffers_pool);
        world.insert_resource(audio);
    }

    fn create_descriptor_set_handle(
        device: Arc<Device>,
        buffers_pool: &mut BuffersPoolResource,
        device_properties_resource: &DevicePropertiesResource,
        push_constant_ranges: &[PushConstantRange],
    ) -> DescriptorSetHandle {
        // Samplers
        DescriptorSetBuilder::new()
            .add_binding(
                DescriptorType::SAMPLER,
                16,
                DescriptorBindingFlags::PARTIALLY_BOUND,
            )
            // Storage Images (aka Draw Image)
            .add_binding(
                DescriptorType::STORAGE_IMAGE,
                2048,
                DescriptorBindingFlags::PARTIALLY_BOUND,
            )
            // Sampled Images (aka Textures), we can resize count of descriptors, we pre-alllocate N descriptors,
            // but we specify that count as unbound (aka variable)
            .add_binding(
                DescriptorType::SAMPLED_IMAGE,
                30_000,
                DescriptorBindingFlags::PARTIALLY_BOUND
                    | DescriptorBindingFlags::VARIABLE_DESCRIPTOR_COUNT,
            )
            .build(
                device,
                buffers_pool,
                &device_properties_resource.descriptor_buffer_properties,
                push_constant_ranges,
                ShaderStageFlags::COMPUTE
                    | ShaderStageFlags::FRAGMENT
                    | ShaderStageFlags::MESH_EXT
                    | ShaderStageFlags::TASK_EXT,
            )
    }
}
