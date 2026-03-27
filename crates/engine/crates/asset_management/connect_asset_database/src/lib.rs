use std::collections::HashMap;

use bevy_ecs::resource::Resource;
use connect_shared::TextureKey;
use slotmap::{Key, SlotMap};
use uuid::Uuid;

type AssetName = String;

#[derive(Default)]
pub struct AssetCategory<TKey: Key> {
    pub textures: SlotMap<TKey, Uuid>,
    pub name_lookup_table: HashMap<AssetName, TKey>,
}

#[derive(Resource)]
pub struct AssetDatabase {
    pub textures: AssetCategory<TextureKey>,
    pub models: AssetCategory<TextureKey>,
    pub materials: AssetCategory<TextureKey>,
    //pub models: SlotMap<TextureKey, Uuid>,
    //pub materials: SlotMap<TextureKey, Uuid>,
}

impl AssetDatabase {
    pub fn new() -> Self {
        AssetDatabase {
            textures: Default::default(),
            models: Default::default(),
            materials: Default::default(),
        }
    }
}
