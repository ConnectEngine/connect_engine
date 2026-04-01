use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use bevy_ecs::{entity::Entity, resource::Resource};
use connect_renderer::{MaterialReference, MeshBufferReference, TextureReference};
use connect_shared::{MaterialKey, MeshBufferKey, TextureKey};
use slotmap::{Key, SlotMap};
use uuid::Uuid;

type AssetPath = String;

#[derive(Clone, Default, Hash, PartialEq, Eq)]
pub struct AssetStatus {
    pub loaded: bool,
    pub asset_entity: Option<Entity>,
}

#[derive(Clone, Default, Hash, PartialEq, Eq)]
pub struct Model<TKey: Key> {
    pub assets_folder_path_buf: PathBuf,
    // TODO: Handle material == None
    pub meshes_dependencies: Vec<MeshAsset<TKey>>,
    pub asset_status: AssetStatus,
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
    pub assets_path_buf: PathBuf,
    pub textures_dependencies: Vec<TextureReference>,
    pub asset_status: AssetStatus,
}

#[derive(Clone, Default, Hash, PartialEq, Eq)]
pub struct TextureAsset<TKey: Key> {
    pub key: TKey,
    pub assets_path_buf: PathBuf,
    pub asset_status: AssetStatus,
}

#[derive(Default)]
pub struct AssetCategory<TKey: Key> {
    pub textures: SlotMap<TKey, Uuid>,
    pub name_lookup_table: HashMap<AssetPath, TKey>,
}

#[derive(Resource)]
pub struct AssetDatabase {
    pub models: Vec<Model<MeshBufferKey>>,
    pub materials: Vec<MaterialAsset<MaterialKey>>,
    pub textures: Vec<TextureAsset<TextureKey>>,
}

impl AssetDatabase {
    pub fn new() -> Self {
        AssetDatabase {
            models: Default::default(),
            textures: Default::default(),
            materials: Default::default(),
        }
    }

    pub fn get_model_asset_entity(&self, name: &str) -> Entity {
        let found_model_asset = self
            .models
            .iter()
            .find(|model_asset| model_asset.assets_folder_path_buf.eq(name));

        found_model_asset
            .unwrap()
            .asset_status
            .asset_entity
            .unwrap()
    }

    pub fn update_model_asset_entity(
        &mut self,
        model_asset_path: &Path,
        asset_entity: Option<Entity>,
    ) {
        let found_model = self
            .models
            .iter_mut()
            .find(|model| model.assets_folder_path_buf.eq(model_asset_path));

        if let Some(found_model) = found_model {
            found_model.asset_status.asset_entity = asset_entity;
        }
    }

    pub fn track_model(
        &mut self,
        meshes_dependencies: Vec<MeshAsset<MeshBufferKey>>,
        mut path_buf: PathBuf,
    ) {
        println!("Tracking model: {}", path_buf.display());
        let model = Model {
            assets_folder_path_buf: path_buf,
            meshes_dependencies,
            asset_status: AssetStatus::default(),
        };

        self.models.push(model);
    }

    pub fn track_material(
        &mut self,
        material_reference: MaterialReference,
        mut path_buf: PathBuf,
        textures: Vec<TextureReference>,
    ) {
        println!("Tracking material: {}", path_buf.display());
        let material = MaterialAsset {
            key: material_reference.key,
            assets_path_buf: path_buf,
            textures_dependencies: textures,
            asset_status: AssetStatus::default(),
        };

        self.materials.push(material);
    }

    pub fn track_texture(&mut self, texture_reference: TextureReference, mut path_buf: PathBuf) {
        println!("Tracking texture: {}", path_buf.display());
        let texture = TextureAsset {
            key: texture_reference.key,
            assets_path_buf: path_buf,
            asset_status: AssetStatus::default(),
        };

        self.textures.push(texture);
    }
}
