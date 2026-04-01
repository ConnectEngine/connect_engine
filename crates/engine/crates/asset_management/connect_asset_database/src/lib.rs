use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use bevy_ecs::resource::Resource;
use connect_renderer::{MaterialReference, MeshBufferReference, TextureReference};
use connect_shared::{MaterialKey, MeshBufferKey, TextureKey};
use slotmap::{Key, SlotMap};
use uuid::Uuid;

type AssetPath = String;

#[derive(Clone, Default, Hash, PartialEq, Eq)]
pub struct Model<TKey: Key> {
    pub path_buf: PathBuf,
    // TODO: Handle material == None
    pub meshes_dependencies: Vec<MeshAsset<TKey>>,
}

#[derive(Clone, Default, Hash, PartialEq, Eq)]
pub struct MeshAsset<TKey: Key> {
    pub key: TKey,
    // TODO: Handle material == None
    pub mesh_buffer_reference: MeshBufferReference,
    pub material_dependency: Option<MaterialReference>,
    pub loaded: bool,
}

#[derive(Clone, Default, Hash, PartialEq, Eq)]
pub struct MaterialAsset<TKey: Key> {
    pub key: TKey,
    pub path: PathBuf,
    pub textures_dependencies: Vec<TextureReference>,
    pub loaded: bool,
}

#[derive(Clone, Default, Hash, PartialEq, Eq)]
pub struct TextureAsset<TKey: Key> {
    pub key: TKey,
    pub path: PathBuf,
    pub loaded: bool,
}

#[derive(Default)]
pub struct AssetCategory<TKey: Key> {
    pub textures: SlotMap<TKey, Uuid>,
    pub name_lookup_table: HashMap<AssetPath, TKey>,
}

#[derive(Resource)]
pub struct AssetDatabase {
    pub models: HashSet<Model<MeshBufferKey>>,
    pub materials: HashSet<MaterialAsset<MaterialKey>>,
    pub textures: HashSet<TextureAsset<TextureKey>>,
}

impl AssetDatabase {
    pub fn new() -> Self {
        AssetDatabase {
            models: Default::default(),
            textures: Default::default(),
            materials: Default::default(),
        }
    }

    pub fn track_model(
        &mut self,
        meshes_dependencies: Vec<MeshAsset<MeshBufferKey>>,
        mut path_buf: PathBuf,
    ) {
        path_buf = Self::trim_extensions_from_path(path_buf);

        println!("Tracking model: {}", path_buf.display());
        let model = Model {
            path_buf,
            meshes_dependencies,
        };

        self.models.insert(model);
    }

    pub fn track_material(
        &mut self,
        material_reference: MaterialReference,
        mut path_buf: PathBuf,
        textures: Vec<TextureReference>,
    ) {
        path_buf = Self::trim_extensions_from_path(path_buf);

        println!("Tracking material: {}", path_buf.display());
        let material = MaterialAsset {
            key: material_reference.key,
            path: path_buf,
            textures_dependencies: textures,
            loaded: true,
        };

        self.materials.insert(material);
    }

    pub fn track_texture(&mut self, texture_reference: TextureReference, mut path_buf: PathBuf) {
        path_buf = Self::trim_extensions_from_path(path_buf);

        println!("Tracking texture: {}", path_buf.display());
        let texture = TextureAsset {
            key: texture_reference.key,
            path: path_buf,
            loaded: true,
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
