use bevy_ecs::{
    entity::Entity,
    system::{Commands, Res, ResMut, SystemParam},
};
use math::Vec3;
use renderer::MeshBuffersPool;

use crate::engine::ecs::physics::{PhysicsManager, RigidBody};

use renderer::*;

#[derive(SystemParam)]
pub struct Physics<'w, 's> {
    commands: Commands<'w, 's>,
    pub(crate) physics_manager: ResMut<'w, PhysicsManager>,
    mesh_buffers_pool: Res<'w, MeshBuffersPool>,
}

impl<'w, 's> Physics<'w, 's> {
    pub fn create_convex_mesh_collider_from_mesh(
        &mut self,
        target_entity: Entity,
        mesh: Mesh,
        rigid_body: RigidBody,
    ) {
        let mesh_buffer_reference = mesh.mesh_buffer_reference;
        let mesh_buffer = self
            .mesh_buffers_pool
            .get_mesh_buffer(mesh_buffer_reference)
            .unwrap();

        let collider = self
            .physics_manager
            .create_convex_mesh_collider(&mesh_buffer.mesh_data, rigid_body.rigid_body_handle);

        let mut entity_commands = self.commands.entity(target_entity);
        entity_commands.insert(collider);
    }

    pub fn create_box_collider(
        &mut self,
        target_entity: Option<Entity>,
        scale: Vec3,
        position: Vec3,
    ) {
        let collider = self
            .physics_manager
            .create_box_collider(scale.to_array(), position.to_array());

        if let Some(target_entity) = target_entity {
            let mut entity_commands = self.commands.entity(target_entity);
            entity_commands.insert(collider);
        } else {
            self.commands.spawn(collider);
        }
    }

    pub fn create_rigid_body(&mut self, target_entity: Entity, world_position: Vec3) -> RigidBody {
        let rigid_body = self
            .physics_manager
            .create_rigid_body(world_position.to_array(), None);

        let mut entity_commands = self.commands.entity(target_entity);
        entity_commands.insert(rigid_body);

        rigid_body
    }
}
