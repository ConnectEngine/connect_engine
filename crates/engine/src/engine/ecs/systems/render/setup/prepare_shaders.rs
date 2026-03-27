use bevy_ecs::system::{Commands, Res, ResMut};
use vulkanite::vk::{rs::*, *};

use crate::engine::utils::load_shader;

use connect_renderer::*;

pub fn prepare_shaders_system(
    mut commands: Commands,
    vulkan_ctx_resource: Res<VulkanContextResource>,
    render_context: ResMut<RendererContextResource>,
    mut renderer_resources: ResMut<RendererResources>,
    descriptor_set_handle: Res<DescriptorSetHandle>,
    mut buffers_pool: ResMut<BuffersPool>,
    mut mesh_buffers_pool: ResMut<MeshBuffersPool>,
) {
    let device = vulkan_ctx_resource.device;

    let descriptor_set_layouts = [descriptor_set_handle.get_descriptor_set_layout()];
    let push_constant_ranges = descriptor_set_handle.push_contant_ranges.as_slice();

    let mesh_shader_path = r"intermediate\shaders\mesh.slang.spv";
    let shaders_info = [
        ShaderInfo {
            path: r"intermediate\shaders\gradient.slang.spv",
            flags: ShaderCreateFlagsEXT::empty(),
            stage: ShaderStageFlags::Compute,
            next_stage: ShaderStageFlags::empty(),
            descriptor_layouts: &descriptor_set_layouts,
            push_constant_ranges: Some(push_constant_ranges),
        },
        ShaderInfo {
            path: mesh_shader_path,
            flags: ShaderCreateFlagsEXT::LinkStage,
            stage: ShaderStageFlags::TaskEXT,
            next_stage: ShaderStageFlags::MeshEXT,
            descriptor_layouts: &descriptor_set_layouts,
            push_constant_ranges: Some(push_constant_ranges),
        },
        ShaderInfo {
            path: mesh_shader_path,
            flags: ShaderCreateFlagsEXT::LinkStage,
            stage: ShaderStageFlags::MeshEXT,
            next_stage: ShaderStageFlags::Fragment,
            descriptor_layouts: &descriptor_set_layouts,
            push_constant_ranges: Some(push_constant_ranges),
        },
        ShaderInfo {
            path: mesh_shader_path,
            flags: ShaderCreateFlagsEXT::LinkStage,
            stage: ShaderStageFlags::Fragment,
            next_stage: ShaderStageFlags::empty(),
            descriptor_layouts: &descriptor_set_layouts,
            push_constant_ranges: Some(push_constant_ranges),
        },
    ];

    let created_shaders = create_shaders(device, &shaders_info);

    renderer_resources.gradient_compute_shader_object = created_shaders[0];
    renderer_resources.task_shader_object = created_shaders[1];
    renderer_resources.mesh_shader_object = created_shaders[2];
    renderer_resources.fragment_shader_object = created_shaders[3];

    // TODO: Move to the other place.
    let materials_data_buffer_reference = buffers_pool.create_buffer(
        1024 * 1024 * 64,
        BufferUsageFlags::ShaderDeviceAddress | BufferUsageFlags::TransferDst,
        BufferVisibility::HostVisible,
        None,
        Some("Materials Data Buffer".to_string()),
    );
    let mut instance_objects_buffers = Vec::with_capacity(render_context.frame_overlap);
    for instances_objects_buffer_index in 0..instance_objects_buffers.capacity() {
        let instance_objects_buffer_reference = buffers_pool.create_buffer(
            std::mem::size_of::<InstanceObject>() * 1_000_000,
            BufferUsageFlags::ShaderDeviceAddress | BufferUsageFlags::TransferDst,
            BufferVisibility::HostVisible,
            Some(MemoryPropertyFlags::LazilyAllocated),
            Some(std::format!(
                "Instances Objects Buffer {}",
                instances_objects_buffer_index
            )),
        );

        instance_objects_buffers.push(instance_objects_buffer_reference);
    }

    let mut scene_data_buffers = Vec::with_capacity(render_context.frame_overlap);
    for scene_data_buffer_index in 0..scene_data_buffers.capacity() {
        let scene_data_buffer_reference = buffers_pool.create_buffer(
            std::mem::size_of::<SceneData>(),
            BufferUsageFlags::ShaderDeviceAddress | BufferUsageFlags::TransferDst,
            BufferVisibility::HostVisible,
            None,
            Some(std::format!(
                "Scene Data Buffer {}",
                scene_data_buffer_index
            )),
        );

        scene_data_buffers.push(scene_data_buffer_reference);
    }

    let mesh_objects_buffer_reference = buffers_pool.create_buffer(
        std::mem::size_of::<MeshObject>() * 8192,
        BufferUsageFlags::ShaderDeviceAddress | BufferUsageFlags::TransferDst,
        BufferVisibility::DeviceOnly,
        None,
        Some("Mesh Objects Buffer".to_string()),
    );

    renderer_resources.resources_pool.instances_buffer =
        Some(SwappableBuffer::new(instance_objects_buffers));
    renderer_resources.resources_pool.scene_data_buffer =
        Some(SwappableBuffer::new(scene_data_buffers));

    renderer_resources.materials_data_buffer_reference = materials_data_buffer_reference;
    let materials_pool = MaterialsPool::new(
        materials_data_buffer_reference
            .get_buffer_info()
            .device_address,
        2048,
    );
    commands.insert_resource(materials_pool);

    renderer_resources.mesh_objects_buffer_reference = mesh_objects_buffer_reference;
    mesh_buffers_pool.set_mesh_objects_buffer_reference(mesh_objects_buffer_reference);
}

fn create_shaders(device: Device, shader_infos: &[ShaderInfo]) -> Vec<ShaderObject> {
    let shader_codes: Vec<Vec<u8>> = shader_infos
        .iter()
        .map(|shader_info| load_shader(shader_info.path))
        .collect();

    let shader_create_infos: Vec<_> = shader_infos
        .iter()
        .zip(shader_codes.as_slice())
        .map(|(shader_info, shader_code)| {
            ShaderCreateInfoEXT::default()
                .flags(shader_info.flags)
                .code(shader_code)
                .name(Some(c"main"))
                .stage(shader_info.stage)
                .next_stage(shader_info.next_stage)
                .code_type(ShaderCodeTypeEXT::Spirv)
                .set_layouts(shader_info.descriptor_layouts)
                .push_constant_ranges(shader_info.push_constant_ranges.unwrap_or_default())
        })
        .collect();

    let (_status, shaders): (_, Vec<ShaderEXT>) =
        device.create_shaders_ext(&shader_create_infos).unwrap();

    shaders
        .into_iter()
        .zip(shader_infos.iter().as_slice())
        .map(|(shader, shader_info)| ShaderObject::new(Some(shader), shader_info.stage))
        .collect()
}
