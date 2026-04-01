use bevy_ecs::{
    hierarchy::ChildOf,
    query::{Changed, With},
    system::{Query, ResMut},
};
use connect_math::{Mat4, Quat, Vec3};
use connect_shared::GlobalTransform;

use crate::engine::{Physics, RigidBody, ecs::physics::PhysicsManager};
use connect_shared::*;

pub fn physics_tick_system(mut physics: ResMut<PhysicsManager>) {
    physics.step();
}

pub fn physics_update_global_transforms(
    physics: Physics,
    mut rigid_bodies_query: Query<(&RigidBody, &mut GlobalTransform)>,
) {
    rigid_bodies_query
        .par_iter_mut()
        .for_each(|(rigid_body, mut global_transform)| {
            let world_position = rigid_body.get_world_position(&physics);
            let world_rotation = rigid_body.get_world_rotation(&physics);

            let (scale, _, _) = global_transform.0.to_scale_rotation_translation();

            global_transform.0 = Mat4::from_scale_rotation_translation(
                scale,
                Quat::from_array(world_rotation.to_array()),
                Vec3::from_array(world_position.to_array()),
            );
        });
}

pub fn physics_update_local_transforms(
    mut local_transform_query: Query<
        (&mut LocalTransform, &GlobalTransform, Option<&ChildOf>),
        (With<RigidBody>, Changed<GlobalTransform>),
    >,
    global_transform_query: Query<&GlobalTransform>,
) {
    local_transform_query.par_iter_mut().for_each(
        |(mut local_transform, global_transform, parent)| {
            let new_local_mat = if let Some(parent) = parent {
                if let Ok(parent_global) = global_transform_query.get(parent.0) {
                    parent_global.0.inverse() * global_transform.0
                } else {
                    global_transform.0
                }
            } else {
                global_transform.0
            };

            let (new_scale, new_rotation, new_translation) =
                new_local_mat.to_scale_rotation_translation();

            local_transform.local_position = new_translation;
            local_transform.local_rotation = new_rotation;
            local_transform.local_scale = new_scale;
        },
    );
}
