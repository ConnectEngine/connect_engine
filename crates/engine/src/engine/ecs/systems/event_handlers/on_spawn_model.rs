use bevy_ecs::{hierarchy::ChildOf, name::Name, observer::On, system::Commands};
use math::{Quat, Vec3};
use renderer::Mesh;

use crate::engine::{
    components::local_transform::{GlobalTransform, LocalTransform},
    events::SpawnEvent,
};

pub fn on_spawn_mesh_system(spawn_event: On<SpawnEvent>, mut commands: Commands) {
    let scene_transform = LocalTransform {
        local_position: Vec3::ZERO,
        local_rotation: Quat::IDENTITY,
        local_scale: Vec3::ONE,
    };
    let scene_global_transform = GlobalTransform(scene_transform.local_to_world_matrix());

    let mut scene_entity_cmds =
        commands.spawn((Name::new("Scene"), scene_global_transform, scene_transform));

    if let Some(parent_entity_id) = spawn_event.parent_entity {
        scene_entity_cmds.insert(ChildOf(parent_entity_id));
    };

    let scene_entity_id = scene_entity_cmds.id();

    let mut spawned_entities = Vec::with_capacity(spawn_event.spawn_records.len());

    for spawn_event_record in spawn_event.spawn_records.iter() {
        let basic_components = (
            GlobalTransform(spawn_event_record.transform.local_to_world_matrix()),
            spawn_event_record.transform,
        );

        let mut spawned_entity_cmds = commands.spawn(basic_components);
        spawned_entities.push(spawned_entity_cmds.id());

        let mut name = Name::new(std::format!(
            "Entity ID: {}",
            spawn_event_record.name.as_str()
        ));

        if let Some(mesh_buffer_reference) = spawn_event_record.mesh_buffer_reference {
            let mesh = Mesh {
                mesh_buffer_reference,
                material_reference: unsafe {
                    spawn_event_record.material_reference.unwrap_unchecked()
                },
            };
            name.set(std::format!(
                "Mesh ID: {}",
                spawn_event_record.name.as_str()
            ));

            spawned_entity_cmds.insert(mesh);
        }

        let parent = if let Some(parent_index) = spawn_event_record.parent_index {
            ChildOf(spawned_entities[parent_index])
        } else {
            ChildOf(scene_entity_id)
        };

        spawned_entity_cmds.insert((name, parent));
    }
}
