use std::path::PathBuf;

use bevy_ecs::{entity::Entity, event::Event};
use renderer::{MaterialReference, MeshBufferReference};
use shared::LocalTransform;

#[derive(Event)]
pub struct LoadModelEvent {
    pub path: PathBuf,
    pub parent_entity: Option<Entity>,
}

#[derive(Clone, Default)]
pub struct SpawnEventRecord {
    pub name: String,
    pub parent_index: Option<usize>,
    pub mesh_buffer_reference: Option<MeshBufferReference>,
    pub material_reference: Option<MaterialReference>,
    pub transform: LocalTransform,
}

#[derive(Default, Event)]
pub struct SpawnEvent {
    pub spawn_records: Vec<SpawnEventRecord>,
    pub parent_entity: Option<Entity>,
}
