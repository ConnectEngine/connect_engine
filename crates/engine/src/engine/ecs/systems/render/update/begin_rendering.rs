use bevy_ecs::system::{Res, ResMut};
use vulkan::{Device, vk::*};

use connect_renderer::*;

pub fn begin_rendering_system(
    vulkan_context_resource: Res<VulkanContextResource>,
    render_context: Res<RendererContextResource>,
    renderer_resources: Res<RendererResources>,
    descriptor_set_handle: Res<DescriptorSetHandle>,
    textures_pool: ResMut<TexturesPoolResource>,
    mut frame_context: ResMut<FrameContextResource>,
) {
    let device = vulkan_context_resource.device.as_ref();
    let frame_data = render_context.get_current_frame_data();

    let command_buffer = frame_data.command_group.command_buffer;
    frame_context.command_buffer = Some(command_buffer);
    frame_context.draw_texture_reference = frame_data.draw_texture_reference;
    frame_context.depth_texture_reference = frame_data.depth_texture_reference;

    let command_buffer_begin_info =
        create_command_buffer_begin_info(CommandBufferUsageFlags::ONE_TIME_SUBMIT);

    unsafe {
        device
            .begin_command_buffer(command_buffer, &command_buffer_begin_info)
            .unwrap();
    }

    let draw_image = textures_pool
        .get_image(frame_context.draw_texture_reference)
        .unwrap();
    let depth_image = textures_pool
        .get_image(frame_context.depth_texture_reference)
        .unwrap();

    transition_image(
        device,
        command_buffer,
        draw_image.image,
        ImageLayout::UNDEFINED,
        ImageLayout::GENERAL,
        PipelineStageFlags2::BLIT,
        PipelineStageFlags2::COMPUTE_SHADER,
        AccessFlags2::TRANSFER_READ,
        AccessFlags2::SHADER_STORAGE_WRITE,
        draw_image.image_aspect_flags,
        frame_context
            .draw_texture_reference
            .texture_metadata
            .mip_levels_count,
    );
    transition_image(
        device,
        command_buffer,
        depth_image.image,
        ImageLayout::UNDEFINED,
        ImageLayout::GENERAL,
        PipelineStageFlags2::LATE_FRAGMENT_TESTS,
        PipelineStageFlags2::EARLY_FRAGMENT_TESTS,
        AccessFlags2::DEPTH_STENCIL_ATTACHMENT_WRITE,
        AccessFlags2::DEPTH_STENCIL_ATTACHMENT_WRITE,
        depth_image.image_aspect_flags,
        frame_context
            .depth_texture_reference
            .texture_metadata
            .mip_levels_count,
    );

    let draw_image_extent3d = draw_image.extent;
    let draw_image_extent2d = Extent2D {
        width: draw_image_extent3d.width,
        height: draw_image_extent3d.height,
    };

    let instance_objects_buffer_reference = renderer_resources
        .resources_pool
        .instances_buffer
        .as_ref()
        .unwrap()
        .get_current_buffer();
    let device_address_instance_objects_buffer = instance_objects_buffer_reference
        .get_buffer_info()
        .device_address;

    let scene_data_buffer_reference = renderer_resources
        .resources_pool
        .scene_data_buffer
        .as_ref()
        .unwrap()
        .get_current_buffer();
    let device_address_scene_data_buffer =
        scene_data_buffer_reference.get_buffer_info().device_address;

    let mesh_push_constant = GraphicsPushConstant {
        device_address_scene_data: device_address_scene_data_buffer,
        device_address_instance_object: device_address_instance_objects_buffer,
        draw_image_index: frame_context.draw_texture_reference.get_index(),
        ..Default::default()
    };

    let pipeline_layout = descriptor_set_handle.get_pipeline_layout();
    let descriptor_buffer_info = descriptor_set_handle.get_buffer_info();

    unsafe {
        device.cmd_push_constants(
            command_buffer,
            pipeline_layout,
            ShaderStageFlags::MESH_EXT
                | ShaderStageFlags::FRAGMENT
                | ShaderStageFlags::COMPUTE
                | ShaderStageFlags::TASK_EXT,
            Default::default(),
            bytemuck::bytes_of(&mesh_push_constant),
        );
    }

    draw_gradient(
        device,
        renderer_resources.as_ref(),
        command_buffer,
        draw_image_extent2d,
        pipeline_layout,
        descriptor_buffer_info.device_address,
    );

    transition_image(
        device,
        command_buffer,
        draw_image.image,
        ImageLayout::GENERAL,
        ImageLayout::GENERAL,
        PipelineStageFlags2::COMPUTE_SHADER,
        PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
        AccessFlags2::SHADER_STORAGE_WRITE,
        AccessFlags2::COLOR_ATTACHMENT_READ,
        draw_image.image_aspect_flags,
        frame_context
            .draw_texture_reference
            .texture_metadata
            .mip_levels_count,
    );

    let color_attachment_infos = [RenderingAttachmentInfo {
        image_view: draw_image.image_view,
        image_layout: ImageLayout::GENERAL,
        resolve_mode: ResolveModeFlags::NONE,
        load_op: AttachmentLoadOp::LOAD,
        store_op: AttachmentStoreOp::STORE,
        ..Default::default()
    }];
    let depth_attachment_info = &RenderingAttachmentInfo {
        image_view: depth_image.image_view,
        image_layout: ImageLayout::GENERAL,
        resolve_mode: ResolveModeFlags::NONE,
        load_op: AttachmentLoadOp::CLEAR,
        store_op: AttachmentStoreOp::STORE,
        clear_value: ClearValue {
            depth_stencil: Default::default(),
        },
        ..Default::default()
    };

    let rendering_info = RenderingInfo {
        render_area: Rect2D {
            extent: draw_image_extent2d,
            ..Default::default()
        },
        layer_count: 1,
        color_attachment_count: color_attachment_infos.len() as _,
        color_attachments: color_attachment_infos.as_ptr(),
        depth_attachment: depth_attachment_info as *const _,
        ..Default::default()
    };

    unsafe {
        device.cmd_begin_rendering(command_buffer, &rendering_info);
    }

    let viewports = [Viewport {
        width: draw_image_extent2d.width as _,
        height: -(draw_image_extent2d.height as f32),
        min_depth: 0.0,
        max_depth: 1.0,
        y: draw_image_extent2d.height as f32,
        ..Default::default()
    }];
    let scissors = [Rect2D {
        extent: draw_image_extent2d,
        ..Default::default()
    }];

    unsafe {
        device.cmd_set_viewport_with_count(command_buffer, &viewports);
        device.cmd_set_scissor_with_count(command_buffer, &scissors);

        device.cmd_set_cull_mode(command_buffer, CullModeFlags::BACK);
        device.cmd_set_front_face(command_buffer, FrontFace::COUNTER_CLOCKWISE);
        device.cmd_set_primitive_topology(command_buffer, PrimitiveTopology::TRIANGLE_LIST);
        vulkan::vk::ExtShaderObjectExtensionDeviceCommands::cmd_set_polygon_mode_ext(
            device,
            command_buffer,
            PolygonMode::FILL,
        );
        device.cmd_set_primitive_restart_enable(command_buffer, false);
        device.cmd_set_rasterizer_discard_enable(command_buffer, false);
        vulkan::vk::ExtShaderObjectExtensionDeviceCommands::cmd_set_rasterization_samples_ext(
            device,
            command_buffer,
            SampleCountFlags::_1,
        );

        device.cmd_set_depth_test_enable(command_buffer, true);
        device.cmd_set_depth_bias_enable(command_buffer, false);
        device.cmd_set_depth_compare_op(command_buffer, CompareOp::GREATER_OR_EQUAL);
        device.cmd_set_depth_bounds_test_enable(command_buffer, false);
        device.cmd_set_depth_bounds(command_buffer, 0.0, 1.0);
        device.cmd_set_stencil_test_enable(command_buffer, false);

        vulkan::vk::ExtShaderObjectExtensionDeviceCommands::cmd_set_alpha_to_coverage_enable_ext(
            device,
            command_buffer,
            false,
        );
        vulkan::vk::ExtShaderObjectExtensionDeviceCommands::cmd_set_sample_mask_ext(
            device,
            command_buffer,
            SampleCountFlags::_1,
            Some(&SampleMask::MAX),
        );
    }

    let color_component_flags = [ColorComponentFlags::all()];
    unsafe {
        vulkan::vk::ExtShaderObjectExtensionDeviceCommands::cmd_set_color_write_mask_ext(
            device,
            command_buffer,
            Default::default(),
            &color_component_flags,
        );
    }

    let vertex_bindings_descriptions: [VertexInputBindingDescription2EXT; 0] = [];
    let vertex_attributes: [VertexInputAttributeDescription2EXT; 0] = [];
    unsafe {
        vulkan::vk::ExtShaderObjectExtensionDeviceCommands::cmd_set_vertex_input_ext(
            device,
            command_buffer,
            &vertex_bindings_descriptions,
            &vertex_attributes,
        );
    }

    let shader_stages = [
        ShaderStageFlags::VERTEX,
        renderer_resources.task_shader_object.stage,
        renderer_resources.mesh_shader_object.stage,
        renderer_resources.fragment_shader_object.stage,
    ];
    let shaders = [
        ShaderEXT::null(),
        renderer_resources.task_shader_object.shader,
        renderer_resources.mesh_shader_object.shader,
        renderer_resources.fragment_shader_object.shader,
    ];

    let descriptor_binding_info = DescriptorBufferBindingInfoEXTBuilder::default()
        .usage(BufferUsageFlags::RESOURCE_DESCRIPTOR_BUFFER_EXT)
        .address(descriptor_buffer_info.device_address)
        .build();
    let descriptor_binding_infos = [descriptor_binding_info];
    unsafe {
        device.cmd_bind_descriptor_buffers_ext(command_buffer, &descriptor_binding_infos);
    }

    let buffer_indices = [0];
    let offsets = [0];

    unsafe {
        device.cmd_set_descriptor_buffer_offsets_ext(
            command_buffer,
            PipelineBindPoint::GRAPHICS,
            pipeline_layout,
            Default::default(),
            &buffer_indices,
            &offsets,
        );
    }

    unsafe {
        device.cmd_bind_shaders_ext(command_buffer, shader_stages.as_slice(), shaders.as_slice());
    }
}

fn draw_gradient(
    device: &Device,
    renderer_resources: &RendererResources,
    command_buffer: CommandBuffer,
    draw_extent: Extent2D,
    pipeline_layout: PipelineLayout,
    descriptor_buffer_device_address: DeviceAddress,
) {
    let gradient_compute_shader_object = renderer_resources.gradient_compute_shader_object;

    let stages = [gradient_compute_shader_object.stage];
    let shaders = [gradient_compute_shader_object.shader];

    unsafe {
        device.cmd_bind_shaders_ext(command_buffer, stages.as_slice(), shaders.as_slice());
    }

    let descriptor_binding_info = DescriptorBufferBindingInfoEXTBuilder::default()
        .usage(BufferUsageFlags::RESOURCE_DESCRIPTOR_BUFFER_EXT)
        .address(descriptor_buffer_device_address)
        .build();

    let descriptor_binding_infos = [descriptor_binding_info];
    unsafe {
        device.cmd_bind_descriptor_buffers_ext(command_buffer, &descriptor_binding_infos);
    }

    let buffer_indices = [0];
    let offsets = [0];
    unsafe {
        device.cmd_set_descriptor_buffer_offsets_ext(
            command_buffer,
            PipelineBindPoint::COMPUTE,
            pipeline_layout,
            Default::default(),
            &buffer_indices,
            &offsets,
        );
    }

    unsafe {
        device.cmd_dispatch(
            command_buffer,
            f32::ceil(draw_extent.width as f32 / 16.0) as _,
            f32::ceil(draw_extent.height as f32 / 16.0) as _,
            1,
        );
    }
}
