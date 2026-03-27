use bevy_ecs::system::{Query, Res, ResMut};

use crate::engine::{components::local_transform::GlobalTransform, resources::RendererResources};

use connect_renderer::*;

// TODO: Take into account if GlobalTransform really changed or not and update if necessary.
pub fn collect_instance_objects_system(
    materials_pool: Res<MaterialsPool>,
    mut renderer_resources: ResMut<RendererResources>,
    mesh_query: Query<(&GlobalTransform, &Mesh)>,
    mesh_buffers: ResMut<MeshBuffersPool>,
) {
    let instance_objects_buffer = unsafe {
        renderer_resources
            .resources_pool
            .instances_buffer
            .as_mut()
            .unwrap_unchecked()
    };
    // TODO: TEMP SOLUTION, in the future will be remade into slot based collecting of instance objects.
    instance_objects_buffer.clear();

    for (global_transform, mesh) in mesh_query.iter() {
        let material_info = materials_pool.get_material_info(mesh.material_reference);

        let mesh_buffer = unsafe {
            mesh_buffers
                .get_mesh_buffer(mesh.mesh_buffer_reference)
                .unwrap_unchecked()
        };

        instance_objects_buffer.add_instance_object(InstanceObject {
            model_matrix: global_transform.0.to_cols_array(),
            device_address_mesh_object: mesh_buffer.mesh_object_device_address,
            device_address_material_data: material_info.device_adddress_material_data,
            meshlet_count: mesh_buffer.meshlets_count as _,
            material_type: material_info.material_type as _,
            ..Default::default()
        });
    }

    instance_objects_buffer.prepare_objects_for_writing();
}
