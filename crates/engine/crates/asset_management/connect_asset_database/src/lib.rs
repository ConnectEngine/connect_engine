use std::{
    collections::{HashMap, HashSet},
    path::{self, PathBuf},
};

use bevy_ecs::resource::Resource;
use connect_renderer::{MaterialReference, TextureReference};
use connect_shared::{MaterialKey, TextureKey};
use slotmap::{Key, SlotMap};
use uuid::Uuid;

type AssetPath = String;

#[derive(Clone, Default, Hash, PartialEq, Eq)]
pub struct Material<TKey: Key> {
    pub key: TKey,
    pub path: PathBuf,
    pub textures: Vec<TextureReference>,
}

#[derive(Clone, Default, Hash, PartialEq, Eq)]
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
    pub materials: Vec<Material<MaterialKey>>,
    pub textures: HashSet<Texture<TextureKey>>,
}

impl AssetDatabase {
    pub fn new() -> Self {
        AssetDatabase {
            textures: Default::default(),
            models: Default::default(),
            materials: Default::default(),
        }
    }

    pub fn track_material(
        &mut self,
        material_reference: MaterialReference,
        mut path_buf: PathBuf,
        textures: Vec<TextureReference>,
    ) {
        path_buf = Self::trim_extensions_from_path(path_buf);

        println!("Tracking material: {}", path_buf.display());
        let material = Material {
            key: material_reference.key,
            path: path_buf,
            textures,
        };
    }

    pub fn track_texture(&mut self, texture_reference: TextureReference, mut path_buf: PathBuf) {
        path_buf = Self::trim_extensions_from_path(path_buf);

        println!("Tracking texture: {}", path_buf.display());
        let texture = Texture {
            key: texture_reference.key,
            path: path_buf,
        };

        self.textures.insert(texture);
    }

    fn trim_extensions_from_path(mut path_buf: PathBuf) -> PathBuf {
        while path_buf.extension().is_some() {
            path_buf.set_extension("");
        }

        path_buf
    }
}
