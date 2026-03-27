use bevy_ecs::world::World;
use connect_renderer::*;
use connect_shared::ModelLoader;
use vulkanite::vk::{rs::Device, *};

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

        let device = vulkan_context.device;
        let allocator = vulkan_context.allocator;

        let renderer_resources = RendererResources {
            default_texture_reference: Default::default(),
            fallback_texture_reference: Default::default(),
            default_sampler_reference: Default::default(),
            mesh_objects_buffer_reference: Default::default(),
            gradient_compute_shader_object: Default::default(),
            task_shader_object: Default::default(),
            mesh_shader_object: Default::default(),
            fragment_shader_object: Default::default(),
            model_loader: ModelLoader::new(),
            resources_pool,
            is_printed_scene_hierarchy: true,
            materials_data_buffer_reference: Default::default(),
        };

        let mut buffers_pool = BuffersPool::new(
            device,
            allocator,
            upload_command_group,
            vulkan_context.transfer_queue,
        );
        let textures_pool = TexturesPoolResource::new(device, vulkan_context.allocator);
        let samplers_pool = SamplersPool::new(device);
        let mesh_buffers_pool = MeshBuffersPool::new(Default::default(), 5_120);

        let push_constant_range = PushConstantRange {
            stage_flags: ShaderStageFlags::MeshEXT
                | ShaderStageFlags::Fragment
                | ShaderStageFlags::Compute
                | ShaderStageFlags::TaskEXT,
            offset: Default::default(),
            size: std::mem::size_of::<GraphicsPushConstant>() as _,
        };

        let push_constant_ranges = [push_constant_range];
        let descriptor_set_handle = Self::create_descriptor_set_handle(
            device,
            allocator,
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
        device: Device,
        allocator: vma::Allocator,
        buffers_pool: &mut BuffersPool,
        device_properties_resource: &DevicePropertiesResource,
        push_constant_ranges: &[PushConstantRange],
    ) -> DescriptorSetHandle {
        // Samplers
        DescriptorSetBuilder::new()
            .add_binding(
                DescriptorType::Sampler,
                16,
                DescriptorBindingFlags::PartiallyBound,
            )
            // Storage Images (aka Draw Image)
            .add_binding(
                DescriptorType::StorageImage,
                2048,
                DescriptorBindingFlags::PartiallyBound,
            )
            // Sampled Images (aka Textures), we can resize count of descriptors, we pre-alllocate N descriptors,
            // but we specify that count as unbound (aka variable)
            .add_binding(
                DescriptorType::SampledImage,
                30_000,
                DescriptorBindingFlags::PartiallyBound
                    | DescriptorBindingFlags::VariableDescriptorCount,
            )
            .build(
                device,
                allocator,
                buffers_pool,
                &device_properties_resource.descriptor_buffer_properties,
                push_constant_ranges,
                ShaderStageFlags::Compute
                    | ShaderStageFlags::Fragment
                    | ShaderStageFlags::MeshEXT
                    | ShaderStageFlags::TaskEXT,
            )
    }
}
