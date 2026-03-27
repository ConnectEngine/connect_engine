use bevy_ecs::{
    change_detection::DetectChanges,
    entity::Entity,
    hierarchy::{ChildOf, Children},
    query::{With, Without},
    relationship::RelationshipTarget,
    system::{Local, Query},
    world::Ref,
};
use connect_math::*;

use crate::engine::components::local_transform::{GlobalTransform, LocalTransform};

pub struct TransformsStack {
    pub stack: Vec<(Entity, Mat4, bool)>,
}

impl Default for TransformsStack {
    fn default() -> Self {
        Self {
            stack: Vec::with_capacity(2_048),
        }
    }
}

pub fn propogate_transforms_system(
    mut root_query: Query<
        (Ref<LocalTransform>, &mut GlobalTransform, Option<&Children>),
        Without<ChildOf>,
    >,
    mut child_query: Query<
        (Ref<LocalTransform>, &mut GlobalTransform, Option<&Children>),
        With<ChildOf>,
    >,
    mut transforms_stack: Local<TransformsStack>,
) {
    transforms_stack.stack.clear();

    for (transform, mut global_transform, children) in root_query.iter_mut() {
        let is_dirty = transform.is_changed();

        let matrix = if is_dirty {
            let new_matrix = transform.local_to_world_matrix();
            global_transform.0 = new_matrix;

            new_matrix
        } else {
            global_transform.0
        };

        if let Some(children) = children {
            for child in children.iter() {
                transforms_stack.stack.push((child, matrix, is_dirty));
            }
        }
    }

    while let Some((child_entity, parent_matrix, parent_dirty)) = transforms_stack.stack.pop() {
        if let Ok((transform, mut global_transform, children)) = child_query.get_mut(child_entity) {
            let is_dirty = parent_dirty || transform.is_changed();

            let child_global_matrix = if is_dirty {
                let local_matrix = transform.local_to_world_matrix();
                let child_global_matrix = parent_matrix * local_matrix;

                global_transform.0 = child_global_matrix;

                child_global_matrix
            } else {
                global_transform.0
            };

            if let Some(children) = children {
                for next_child in children.iter() {
                    transforms_stack
                        .stack
                        .push((next_child, child_global_matrix, is_dirty));
                }
            }
        }
    }
}
