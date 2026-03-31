use std::{collections::HashMap, path::PathBuf};

use bevy_ecs::resource::Resource;
use connect_renderer::TextureReference;
use connect_shared::TextureKey;
use slotmap::{Key, SlotMap};
use uuid::Uuid;

type AssetPath = String;

#[derive(Default)]
pub struct Material<TKey: Key> {
    pub key: TKey,
    pub path: PathBuf,
    pub textures: Vec<TextureKey>,
}

#[derive(Clone, Default)]
pub struct Texture<TKey: Key> {
    pub key: TKey,
    pub path: PathBuf,
}

#[derive(Default)]
pub struct AssetCategory<TKey: Key> {
    pub textures: SlotMap<TKey, Uuid>,
    pub name_lookup_table: HashMap<AssetPath, TKey>,
}

#[derive(Resource)]
pub struct AssetDatabase {
    pub models: AssetCategory<TextureKey>,
    pub materials: AssetCategory<TextureKey>,
    pub textures: Vec<Texture<TextureKey>>,
}

impl AssetDatabase {
    pub fn new() -> Self {
        AssetDatabase {
            textures: Default::default(),
            models: Default::default(),
            materials: Default::default(),
        }
    }

    pub fn track_texture(&mut self, texture_reference: TextureReference, mut path_buf: PathBuf) {
        while path_buf.extension().is_some() {
            path_buf.set_extension("");
        }

        println!("Tracking texture: {}", path_buf.display());
        let texture = Texture {
            key: texture_reference.key,
            path: path_buf,
        };

        self.textures.push(texture);
    }
}
