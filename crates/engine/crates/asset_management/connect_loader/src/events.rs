use std::path::PathBuf;

use bevy_ecs::{entity::Entity, event::Event};

use connect_renderer::*;
use connect_shared::*;

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
    // FIXME: Currently, path_buf corresponds (aka "key") to the  `AssetDatabase`'s asset,
    // later should be used a lightweight key (maybe use `slotmap` create), not sure, currently.
    pub asset_path_buf: PathBuf,
    pub spawn_records: Vec<SpawnEventRecord>,
    pub parent_entity: Option<Entity>,
}
