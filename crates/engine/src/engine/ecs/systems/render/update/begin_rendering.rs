use bevy_ecs::system::{Res, ResMut};
use vulkanite::{
    Handle,
    vk::{
        rs::{CommandBuffer, PipelineLayout},
        *,
    },
};

use crate::engine::resources::{FrameContextResource, GraphicsPushConstant, RendererResources};

use connect_renderer::*;

pub fn begin_rendering_system(
    render_context: Res<RendererContextResource>,
    renderer_resources: Res<RendererResources>,
    descriptor_set_handle: Res<DescriptorSetHandle>,
    textures_pool: ResMut<TexturesPoolResource>,
    mut frame_context: ResMut<FrameContextResource>,
) {
    let frame_data = render_context.get_current_frame_data();

    let command_buffer = frame_data.command_group.command_buffer;
    frame_context.command_buffer = Some(command_buffer);
    frame_context.draw_texture_reference = frame_data.draw_texture_reference;
    frame_context.depth_texture_reference = frame_data.depth_texture_reference;

    let command_buffer_begin_info =
        create_command_buffer_begin_info(CommandBufferUsageFlags::OneTimeSubmit);

    command_buffer.begin(&command_buffer_begin_info).unwrap();

    let draw_image = textures_pool
        .get_image(frame_context.draw_texture_reference)
        .unwrap();
    let depth_image = textures_pool
        .get_image(frame_context.depth_texture_reference)
        .unwrap();

    transition_image(
        command_buffer,
        draw_image.image,
        ImageLayout::Undefined,
        ImageLayout::General,
        PipelineStageFlags2::Blit,
        PipelineStageFlags2::ComputeShader,
        AccessFlags2::TransferRead,
        AccessFlags2::ShaderStorageWrite,
        draw_image.image_aspect_flags,
        frame_context
            .draw_texture_reference
            .texture_metadata
            .mip_levels_count,
    );
    transition_image(
        command_buffer,
        depth_image.image,
        ImageLayout::Undefined,
        ImageLayout::General,
        PipelineStageFlags2::LateFragmentTests,
        PipelineStageFlags2::EarlyFragmentTests,
        AccessFlags2::DepthStencilAttachmentWrite,
        AccessFlags2::DepthStencilAttachmentWrite,
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

    command_buffer.push_constants(
        pipeline_layout,
        ShaderStageFlags::MeshEXT
            | ShaderStageFlags::Fragment
            | ShaderStageFlags::Compute
            | ShaderStageFlags::TaskEXT,
        Default::default(),
        size_of::<GraphicsPushConstant>() as u32,
        &mesh_push_constant as *const _ as _,
    );

    draw_gradient(
        renderer_resources.as_ref(),
        command_buffer,
        draw_image_extent2d,
        pipeline_layout,
        descriptor_buffer_info.device_address,
    );

    transition_image(
        command_buffer,
        draw_image.image,
        ImageLayout::General,
        ImageLayout::General,
        PipelineStageFlags2::ComputeShader,
        PipelineStageFlags2::ColorAttachmentOutput,
        AccessFlags2::ShaderStorageWrite,
        AccessFlags2::ColorAttachmentRead,
        draw_image.image_aspect_flags,
        frame_context
            .draw_texture_reference
            .texture_metadata
            .mip_levels_count,
    );

    let color_attachment_infos = [RenderingAttachmentInfo {
        image_view: Some(draw_image.image_view.borrow()),
        image_layout: ImageLayout::General,
        resolve_mode: ResolveModeFlags::None,
        load_op: AttachmentLoadOp::Load,
        store_op: AttachmentStoreOp::Store,
        ..Default::default()
    }];
    let depth_attachment_info = &RenderingAttachmentInfo {
        image_view: Some(depth_image.image_view.borrow()),
        image_layout: ImageLayout::General,
        resolve_mode: ResolveModeFlags::None,
        load_op: AttachmentLoadOp::Clear,
        store_op: AttachmentStoreOp::Store,
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
        p_color_attachments: color_attachment_infos.as_ptr(),
        p_depth_attachment: depth_attachment_info as *const _,
        ..Default::default()
    };

    command_buffer.begin_rendering(&rendering_info);

    let viewports = Viewport {
        width: draw_image_extent2d.width as _,
        height: -(draw_image_extent2d.height as f32),
        min_depth: 0.0,
        max_depth: 1.0,
        y: draw_image_extent2d.height as f32,
        ..Default::default()
    };
    let scissors = Rect2D {
        extent: draw_image_extent2d,
        ..Default::default()
    };

    command_buffer.set_viewport_with_count(&viewports);
    command_buffer.set_scissor_with_count(&scissors);

    command_buffer.set_cull_mode(CullModeFlags::Back);
    command_buffer.set_front_face(FrontFace::CounterClockwise);
    command_buffer.set_primitive_topology(PrimitiveTopology::TriangleList);
    command_buffer.set_polygon_mode_ext(PolygonMode::Fill);
    command_buffer.set_primitive_restart_enable(false);
    command_buffer.set_rasterizer_discard_enable(false);
    command_buffer.set_rasterization_samples_ext(SampleCountFlags::Count1);

    command_buffer.set_depth_test_enable(true);
    command_buffer.set_depth_bias_enable(false);
    command_buffer.set_depth_compare_op(CompareOp::GreaterOrEqual);
    command_buffer.set_depth_bounds_test_enable(false);
    command_buffer.set_depth_bounds(0.0, 1.0);
    command_buffer.set_stencil_test_enable(false);

    command_buffer.set_alpha_to_coverage_enable_ext(false);
    command_buffer.set_sample_mask_ext(SampleCountFlags::Count1, &[SampleMask::MAX]);

    let color_component_flags = [ColorComponentFlags::all()];
    command_buffer.set_color_write_mask_ext(Default::default(), &color_component_flags);

    let vertex_bindings_descriptions = [];
    let vertex_attributes = [];
    command_buffer.set_vertex_input_ext(&vertex_bindings_descriptions, &vertex_attributes);

    let shader_stages = [ShaderStageFlags::Vertex];
    use vulkanite::Dispatcher;

    unsafe {
        let dispatcher = command_buffer.get_dispatcher();
        let vulkan_command = dispatcher
            .get_command_dispatcher()
            .cmd_bind_shaders_ext
            .get();
        vulkan_command(
            Some(command_buffer.borrow()),
            1,
            shader_stages.as_slice().as_ptr().cast(),
            std::ptr::null(),
        );
    }

    let shader_stages = [
        renderer_resources.task_shader_object.stage,
        renderer_resources.mesh_shader_object.stage,
        renderer_resources.fragment_shader_object.stage,
    ];
    let shaders = [
        *renderer_resources.task_shader_object.shader.unwrap(),
        *renderer_resources.mesh_shader_object.shader.unwrap(),
        *renderer_resources.fragment_shader_object.shader.unwrap(),
    ];

    let descriptor_binding_info = DescriptorBufferBindingInfoEXT::default()
        .usage(BufferUsageFlags::ResourceDescriptorBufferEXT)
        .address(descriptor_buffer_info.device_address);
    let descriptor_binding_infos = [descriptor_binding_info];
    command_buffer.bind_descriptor_buffers_ext(&descriptor_binding_infos);

    let buffer_indices = [0];
    let offsets = [0];
    command_buffer.set_descriptor_buffer_offsets_ext(
        PipelineBindPoint::Graphics,
        pipeline_layout,
        Default::default(),
        &buffer_indices,
        &offsets,
    );

    command_buffer.bind_shaders_ext(shader_stages.as_slice(), shaders.as_slice());
}

fn draw_gradient(
    renderer_resources: &RendererResources,
    command_buffer: CommandBuffer,
    draw_extent: Extent2D,
    pipeline_layout: PipelineLayout,
    descriptor_buffer_device_address: DeviceAddress,
) {
    let gradient_compute_shader_object = renderer_resources.gradient_compute_shader_object;

    let stages = [gradient_compute_shader_object.stage];
    let shaders = [gradient_compute_shader_object.shader.unwrap()];

    command_buffer.bind_shaders_ext(stages.as_slice(), shaders.as_slice());

    let descriptor_binding_info = DescriptorBufferBindingInfoEXT::default()
        .usage(BufferUsageFlags::ResourceDescriptorBufferEXT)
        .address(descriptor_buffer_device_address);

    let descriptor_binding_infos = [descriptor_binding_info];
    command_buffer.bind_descriptor_buffers_ext(&descriptor_binding_infos);

    let buffer_indices = [0];
    let offsets = [0];
    command_buffer.set_descriptor_buffer_offsets_ext(
        PipelineBindPoint::Compute,
        pipeline_layout,
        Default::default(),
        &buffer_indices,
        &offsets,
    );

    command_buffer.dispatch(
        f32::ceil(draw_extent.width as f32 / 16.0) as _,
        f32::ceil(draw_extent.height as f32 / 16.0) as _,
        1,
    );
}
