use std::{
    io::Read,
    path::{Path, PathBuf},
};

use asset_database::AssetDatabase;
use bevy_ecs::{
    resource::Resource,
    system::{Res, ResMut},
};
use information::Information;
use shared::{AssetMetadata, AssetsExtensions};
use uuid::Uuid;
use walkdir::WalkDir;

#[derive(Clone, Copy, PartialEq, Eq)]
enum AssetType {
    Model,
    Texture,
    Material,
}

struct AssetToLoad {
    pub uuid: Uuid,
    pub name: String,
    pub path: PathBuf,
}

#[derive(Default, Resource)]
pub struct Loader {
    pub collected_meta_files: Vec<AssetMetadata>,
    pub models_to_load: Vec<AssetToLoad>,
    pub textures_to_load: Vec<AssetToLoad>,
    pub materials_to_load: Vec<AssetToLoad>,
}

impl Loader {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn collect_meta_files(&mut self, assets_folder_path: &Path) {
        for entry in WalkDir::new(assets_folder_path)
            .into_iter()
            .filter_map(|dir_entry| dir_entry.ok())
        {
            if entry.file_type().is_file() {
                if entry
                    .path()
                    .extension()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .eq(AssetsExtensions::META_FILE_EXTENSION)
                {
                    let mut metadata_content = String::new();
                    std::fs::File::open(entry.path())
                        .unwrap()
                        .read_to_string(&mut metadata_content)
                        .unwrap();
                    let meta_file =
                        toml::de::from_str::<AssetMetadata>(metadata_content.as_str()).unwrap();

                    self.collected_meta_files.push(meta_file);
                }
            }
        }
    }

    pub fn resolve_meta_files(&mut self, assset_database: &mut AssetDatabase) {
        self.collected_meta_files
            .drain(..)
            .for_each(|meta_file| match meta_file {
                AssetMetadata::Model(model_asset_metadata) => {
                    self.models_to_load.push(AssetToLoad {
                        uuid: model_asset_metadata.uuid,
                        name: model_asset_metadata.name.clone(),
                        path: ,
                    });
                }
                AssetMetadata::Texture(texture_asset_metadata) => {
                    self.textures_to_load.push(AssetToLoad {
                        uuid: texture_asset_metadata.uuid,
                        name: texture_asset_metadata.name.clone(),
                    });
                }
                AssetMetadata::Material(material_asset_metadata) => {
                    self.materials_to_load.push(AssetToLoad {
                        uuid: material_asset_metadata.uuid,
                        name: material_asset_metadata.name.clone(),
                    });
                }
            });
    }

    pub fn load_assets(&mut self, asset_database: &mut AssetDatabase) {}

    pub fn resolve_path(uuid: Uuid, asset_type: AssetType) {

    }
}

pub fn load_assets_system(
    information: Res<Information>,
    mut loader: ResMut<Loader>,
    mut asset_database: ResMut<AssetDatabase>,
) {
    let editor_application = information.get_editor_application();

    loader.collect_meta_files(editor_application.get_assets_folder_path());
    loader.resolve_meta_files(&mut asset_database);
    loader.load_assets(&mut asset_database);
}
