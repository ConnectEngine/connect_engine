use bevy_ecs::system::{Query, Res, ResMut};
use bytemuck::Pod;
use connect_math::*;

use crate::engine::{
    LocalTransform,
    components::camera::Camera,
    resources::{DirectionalLight, LightProperties, RendererResources, SceneData, SwappableBuffer},
};

use connect_renderer::*;

pub fn update_resources_system(
    render_context: Res<RendererContextResource>,
    mut renderer_resources: ResMut<RendererResources>,
    buffers: ResMut<BuffersPool>,
    mut frame_context: ResMut<frame_context_resource::FrameContextResource>,
    transform_camera_query: Query<(&Camera, &LocalTransform)>,
) {
    let instances_objects_buffer = unsafe {
        renderer_resources
            .resources_pool
            .instances_buffer
            .as_ref()
            .unwrap_unchecked()
    };

    update_buffer_data(instances_objects_buffer, &buffers);

    // TODO: Graceful fallback to black screen, if no cameras on a scene.
    if let Ok((camera, transform)) = transform_camera_query.single() {
        let camera_position = transform.get_local_position();
        let view = Mat4::from_scale_rotation_translation(
            Vec3::ONE,
            transform.get_local_rotation(),
            camera_position,
        )
        .inverse();

        let projection = Mat4::perspective_rh(
            camera.fov.to_radians(),
            render_context.draw_extent.width as f32 / render_context.draw_extent.height as f32,
            camera.clipping_planes.far,
            camera.clipping_planes.near,
        );

        frame_context.world_matrix = projection * view;

        let scene_data_buffer = unsafe {
            renderer_resources
                .resources_pool
                .scene_data_buffer
                .as_mut()
                .unwrap_unchecked()
        };

        let scene_data = SceneData {
            camera_view_matrix: frame_context.world_matrix.to_cols_array(),
            camera_position,
            light_properties: LightProperties {
                ambient_color: Vec4::new(0.1, 0.1, 0.1, 1.0),
                ambient_strength: 0.1,
                specular_strength: 0.7,
                ..Default::default()
            },
            directional_light: DirectionalLight {
                light_color: Vec3::new(0.72, 0.72, 0.93),
                light_position: Vec3::new(0.1, 0.5, 1.0),
                ..Default::default()
            },
            ..Default::default()
        };
        scene_data_buffer.clear();
        scene_data_buffer.add_instance_object(scene_data);
        scene_data_buffer.prepare_objects_for_writing();

        let scene_data_buffer = unsafe {
            renderer_resources
                .resources_pool
                .scene_data_buffer
                .as_ref()
                .unwrap_unchecked()
        };

        update_buffer_data(scene_data_buffer, &buffers);
    }
}

fn update_buffer_data<T: Pod>(buffer_to_update: &SwappableBuffer<T>, buffers: &BuffersPool) {
    let data_to_write = buffer_to_update.get_objects_to_write_as_slice();

    let buffer_to_update_reference = buffer_to_update.get_current_buffer();
    unsafe {
        buffers.transfer_data_to_buffer(
            buffer_to_update_reference,
            data_to_write,
            data_to_write.len(),
        );
    }
}
