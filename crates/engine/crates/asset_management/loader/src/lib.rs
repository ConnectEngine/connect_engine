use std::{
    io::{BufReader, Read},
    path::{Path, PathBuf},
};

use asset_database::AssetDatabase;
use bevy_ecs::{
    resource::Resource,
    system::{Res, ResMut},
};
use information::Information;
use shared::{ArchivedSerializedModel, ArtifactsFoldersNames, AssetMetadata, AssetsExtensions};
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
    pub(crate) collected_meta_files: Vec<AssetMetadata>,
    pub(crate) models_to_load: Vec<AssetToLoad>,
    pub(crate) textures_to_load: Vec<AssetToLoad>,
    pub(crate) materials_to_load: Vec<AssetToLoad>,
}

impl Loader {
    pub fn new() -> Self {
        Default::default()
    }

    pub(crate) fn collect_meta_files(&mut self, assets_folder_path: &Path) {
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

    pub(crate) fn resolve_meta_files(
        &mut self,
        assset_database: &mut AssetDatabase,
        artifacts_folder_path: &Path,
    ) {
        self.collected_meta_files
            .drain(..)
            .for_each(|meta_file| match meta_file {
                AssetMetadata::Model(model_asset_metadata) => {
                    self.models_to_load.push(AssetToLoad {
                        uuid: model_asset_metadata.uuid,
                        name: model_asset_metadata.name.clone(),
                        path: Self::resolve_path(
                            AssetType::Model,
                            &model_asset_metadata.name,
                            model_asset_metadata.uuid,
                            artifacts_folder_path,
                        ),
                    });
                }
                AssetMetadata::Texture(texture_asset_metadata) => {
                    self.textures_to_load.push(AssetToLoad {
                        uuid: texture_asset_metadata.uuid,
                        name: texture_asset_metadata.name.clone(),
                        path: Self::resolve_path(
                            AssetType::Texture,
                            &texture_asset_metadata.name,
                            texture_asset_metadata.uuid,
                            artifacts_folder_path,
                        ),
                    });
                }
                AssetMetadata::Material(material_asset_metadata) => {
                    self.materials_to_load.push(AssetToLoad {
                        uuid: material_asset_metadata.uuid,
                        name: material_asset_metadata.name.clone(),
                        path: Self::resolve_path(
                            AssetType::Material,
                            &material_asset_metadata.name,
                            material_asset_metadata.uuid,
                            artifacts_folder_path,
                        ),
                    });
                }
            });
    }

    pub(crate) fn load_assets(&mut self, asset_database: &mut AssetDatabase) {
        self.models_to_load.drain(..).for_each(|model_to_load| {
            let serialized_model_buf_reader =
                BufReader::new(std::fs::File::open(model_to_load.path).unwrap());

            let archived_serialized_model = rkyv::access::<
                ArchivedSerializedModel,
                rkyv::rancor::Error,
            >(serialized_model_buf_reader.buffer())
            .unwrap();

            archived_serialized_model
                .hierarchy
                .serialized_nodes
                .iter()
                .for_each(|node| {
                    if node.mesh_index.is_some() {
                        let mesh_index = node.mesh_index.unwrap();

                        let serialized_mesh = archived_serialized_model
                            .meshes
                            .get(mesh_index.to_native() as usize)
                            .unwrap();
                    }
                });
        });
    }

    pub(crate) fn resolve_path(
        asset_type: AssetType,
        name: &str,
        uuid: Uuid,
        artifacts_folder_path: &Path,
    ) -> PathBuf {
        let mut path_buf = PathBuf::from(artifacts_folder_path);
        match asset_type {
            AssetType::Model => {
                path_buf.push(ArtifactsFoldersNames::MODELS_FOLDER_NAME);
            }
            AssetType::Texture => {
                path_buf.push(ArtifactsFoldersNames::TEXTURES_FOLDER_NAME);
            }
            AssetType::Material => {
                path_buf.push(ArtifactsFoldersNames::MATERIALS_FOLDER_NAME);
            }
        }

        let uuid_str = uuid.to_string();
        let shard_folder = &uuid_str[0..2];

        path_buf.push(shard_folder);

        path_buf.push(std::format!("{name}_{uuid}"));

        path_buf
    }
}

pub fn load_assets_system(
    information: Res<Information>,
    mut loader: ResMut<Loader>,
    mut asset_database: ResMut<AssetDatabase>,
) {
    let editor_application = information.get_editor_application();

    loader.collect_meta_files(editor_application.get_assets_folder_path());
    loader.resolve_meta_files(
        &mut asset_database,
        information
            .get_editor_application()
            .get_artifacts_folder_path(),
    );
    loader.load_assets(&mut asset_database);
}
