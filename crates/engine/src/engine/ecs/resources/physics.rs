// TODO: MOVE TO THE PROPER FOLDER!

use bevy_ecs::{component::Component, resource::Resource};
use connect_math::*;
use rapier3d::{
    glamx::Quat,
    prelude::{
        CCDSolver, ColliderBuilder, ColliderSet, DefaultBroadPhase, ImpulseJointSet,
        IntegrationParameters, IslandManager, MultibodyJointSet, NarrowPhase, PhysicsPipeline,
        RigidBodyBuilder, RigidBodyHandle, RigidBodySet,
    },
};

use crate::engine::{LocalTransform, Physics};

use connect_renderer::*;

#[derive(Component, Clone, Copy)]
#[require(LocalTransform)]
pub struct Collider {
    pub(crate) collider_handle: rapier3d::prelude::ColliderHandle,
}

#[derive(Component, Clone, Copy)]
#[require(LocalTransform)]
pub struct RigidBody {
    pub(crate) rigid_body_handle: rapier3d::prelude::RigidBodyHandle,
}

impl RigidBody {
    pub fn apply_impulse(&mut self, impulse: Vec3, physics: &mut Physics) {
        let rigid_body = &mut physics.physics_manager.rigid_body_set[self.rigid_body_handle];
        rigid_body.apply_impulse(rapier3d::math::Vec3::from_array(impulse.to_array()), true);
    }

    pub fn get_world_position(&self, physics: &Physics) -> Vec3 {
        let rigid_body = &physics.physics_manager.rigid_body_set[self.rigid_body_handle];

        Vec3::from_array(rigid_body.translation().to_array())
    }

    pub fn get_world_rotation(&self, physics: &Physics) -> Quat {
        let rigid_body = &physics.physics_manager.rigid_body_set[self.rigid_body_handle];

        *rigid_body.rotation()
    }
}

#[derive(Resource)]
pub struct PhysicsManager {
    gravity: rapier3d::math::Vec3,
    rigid_body_set: RigidBodySet,
    collider_set: ColliderSet,
    physics_pipeline: PhysicsPipeline,
    integration_parameters: IntegrationParameters,
    island_manager: IslandManager,
    broad_phase: DefaultBroadPhase,
    narrow_phase: NarrowPhase,
    impulse_joint_set: ImpulseJointSet,
    multibody_joint_set: MultibodyJointSet,
    ccd_solver: CCDSolver,
}

impl PhysicsManager {
    pub fn new() -> Self {
        let gravity = rapier3d::glamx::vec3(0.0, -9.81, 0.0);

        let rigid_body_set = RigidBodySet::new();
        let collider_set = ColliderSet::new();
        let integration_parameters = IntegrationParameters::default();
        let physics_pipeline = PhysicsPipeline::new();
        let island_manager = IslandManager::new();
        let broad_phase = DefaultBroadPhase::new();
        let narrow_phase = NarrowPhase::new();
        let impulse_joint_set = ImpulseJointSet::new();
        let multibody_joint_set = MultibodyJointSet::new();
        let ccd_solver = CCDSolver::new();

        Self {
            gravity,
            rigid_body_set,
            collider_set,
            physics_pipeline,
            integration_parameters,
            island_manager,
            broad_phase,
            narrow_phase,
            impulse_joint_set,
            multibody_joint_set,
            ccd_solver,
        }
    }

    pub fn step(&mut self) {
        let physics_hooks = ();
        let event_handler = ();

        self.physics_pipeline.step(
            self.gravity,
            &self.integration_parameters,
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.rigid_body_set,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            &mut self.ccd_solver,
            &physics_hooks,
            &event_handler,
        );
    }

    // TODO: Later accept Option RigidBody as parameter, for unified and easy to use API.
    pub fn create_convex_mesh_collider(
        &mut self,
        mesh_data: &MeshData,
        rigid_body_handle: RigidBodyHandle,
    ) -> Collider {
        let vertices: Vec<rapier3d::math::Vec3> = mesh_data
            .vertices
            .iter()
            .map(|vertex| rapier3d::math::Vec3::from_array(vertex.position))
            .collect();
        let indices: Vec<[u32; 3]> = mesh_data
            .indices
            .chunks_exact(3)
            .map(|chunk| [chunk[0], chunk[1], chunk[2]])
            .collect();

        let collider =
            ColliderBuilder::convex_decomposition(vertices.as_slice(), indices.as_slice()).build();
        let collider_handle = self.collider_set.insert_with_parent(
            collider,
            rigid_body_handle,
            &mut self.rigid_body_set,
        );

        Collider { collider_handle }
    }

    pub fn create_box_collider(&mut self, scale: [f32; 3], position: [f32; 3]) -> Collider {
        let hscale = Vec3::from_array(scale) / 2.0;
        let collider = ColliderBuilder::cuboid(hscale.x, hscale.y, hscale.z)
            .translation(rapier3d::math::Vec3::from_array(position))
            .build();

        let collider_handle = self.collider_set.insert(collider);

        Collider { collider_handle }
    }

    pub fn create_rigid_body(&mut self, world_position: [f32; 3], mass: Option<f32>) -> RigidBody {
        let rigid_body = RigidBodyBuilder::dynamic()
            // FIXME: Issue with different versions of glam.
            .translation(rapier3d::math::Vec3::from_array(world_position))
            .additional_mass(mass.unwrap_or(1.0))
            .build();
        let rigid_body_handle = self.rigid_body_set.insert(rigid_body);

        RigidBody { rigid_body_handle }
    }
}
