use bevy_ecs::{
    entity::Entity,
    hierarchy::ChildOf,
    name::Name,
    system::{Query, Res, ResMut},
};
use vulkan::vk::*;

use connect_renderer::*;

pub fn render_meshes_system(
    vulkan_context_resource: Res<VulkanContextResource>,
    graphics_entities: Query<&Mesh>,
    entities: Query<(Entity, &Name)>,
    entities_with_parent: Query<&ChildOf>,
    mut renderer_resources: ResMut<RendererResources>,
    descriptor_set_handle: Res<DescriptorSetHandle>,
    frame_context: Res<FrameContextResource>,
) {
    let device = vulkan_context_resource.device.as_ref();
    let command_buffer = frame_context.command_buffer.unwrap();

    if !renderer_resources.is_printed_scene_hierarchy {
        println!("=====================================");

        for (entity, name) in entities.iter() {
            if let Ok(parent) = entities_with_parent.get(entity) {
                println!("Entity: {} | Name: {} | Parent: {}", entity, name, parent.0);
            } else {
                println!("Entity: {} | Name: {}", entity, name);
            }
        }

        println!("=====================================");
    }

    let color_blend_equation = [ColorBlendEquationEXT {
        src_color_blend_factor: BlendFactor::ONE,
        dst_color_blend_factor: BlendFactor::ONE,
        color_blend_op: BlendOp::ADD,
        src_alpha_blend_factor: BlendFactor::ONE,
        dst_alpha_blend_factor: BlendFactor::ZERO,
        alpha_blend_op: BlendOp::ADD,
    }];
    unsafe {
        vulkan::vk::ExtShaderObjectExtensionDeviceCommands::cmd_set_color_blend_equation_ext(
            device,
            command_buffer,
            Default::default(),
            &color_blend_equation,
        );
    }

    let meshes_len = graphics_entities.iter().len();
    for material_type in 0..2 {
        let is_draw_transparent_materials =
            material_type as u32 == MaterialType::Transparent as u32;
        let blend_enables = [Bool32::from(is_draw_transparent_materials)];

        unsafe {
            device.cmd_set_depth_write_enable(command_buffer, !is_draw_transparent_materials);
            vulkan::vk::ExtShaderObjectExtensionDeviceCommands::cmd_set_color_blend_enable_ext(
                device,
                command_buffer,
                Default::default(),
                blend_enables.as_slice(),
            );
        }

        let push_constants = GraphicsPushConstant {
            current_material_type: material_type as _,
            ..Default::default()
        };

        unsafe {
            device.cmd_push_constants(
                command_buffer,
                descriptor_set_handle.get_pipeline_layout(),
                ShaderStageFlags::FRAGMENT
                    | ShaderStageFlags::TASK_EXT
                    | ShaderStageFlags::MESH_EXT
                    | ShaderStageFlags::COMPUTE,
                std::mem::offset_of!(GraphicsPushConstant, current_material_type) as _,
                bytemuck::bytes_of(&push_constants.current_material_type),
            );
        }

        unsafe {
            device.cmd_draw_mesh_tasks_ext(command_buffer, meshes_len as _, 1, 1);
        }
    }

    renderer_resources.is_printed_scene_hierarchy = true;
}
