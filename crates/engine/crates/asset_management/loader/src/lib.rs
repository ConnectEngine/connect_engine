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
use shared::{
    ArchivedSerializedModel, ArtifactsFoldersNames, AssetMetadata, AssetsExtensions, Meshlet,
    Vertex,
};
use uuid::Uuid;
use vulkanite::vk::BufferUsageFlags;
use walkdir::WalkDir;

use renderer::resources::*;

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

    pub(crate) fn load_assets(
        &mut self,
        asset_database: &mut AssetDatabase,
        buffers_pool: &mut BuffersPool,
        mesh_buffers_pool: &mut MeshBuffersPool,
    ) {
        self.models_to_load.drain(..).for_each(|model_to_load| {
            let serialized_model_buf_reader =
                BufReader::new(std::fs::File::open(model_to_load.path).unwrap());

            let archived_serialized_model = rkyv::access::<
                ArchivedSerializedModel,
                rkyv::rancor::Error,
            >(serialized_model_buf_reader.buffer())
            .unwrap();

            let mut mesh_buffers_to_upload =
                Vec::with_capacity(archived_serialized_model.meshes.len());

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

                        let vertex_buffer_reference = Self::create_and_copy_to_buffer(
                            buffers_pool,
                            serialized_mesh.vertices.as_ptr() as *const _,
                            serialized_mesh.vertices.len() * std::mem::size_of::<Vertex>(),
                            std::format!("{}_{}", serialized_mesh.name, stringify!(vertices)),
                        );
                        let vertex_indices_buffer_reference = Self::create_and_copy_to_buffer(
                            buffers_pool,
                            serialized_mesh.indices.as_ptr() as _,
                            serialized_mesh.indices.len() * std::mem::size_of::<u32>(),
                            std::format!("{}_{}", serialized_mesh.name, stringify!(vertex_indices)),
                        );
                        let meshlets_buffer_reference = Self::create_and_copy_to_buffer(
                            buffers_pool,
                            serialized_mesh.meshlets.as_ptr() as _,
                            serialized_mesh.meshlets.len() * std::mem::size_of::<Meshlet>(),
                            std::format!("{}_{}", serialized_mesh.name, stringify!(meshlets)),
                        );

                        let local_indices_buffer_reference = Self::create_and_copy_to_buffer(
                            buffers_pool,
                            serialized_mesh.triangles.as_ptr() as _,
                            serialized_mesh.triangles.len() * std::mem::size_of::<u8>(),
                            std::format!("{}_{}", serialized_mesh.name, stringify!(triangles)),
                        );

                        let mesh_data = MeshData {
                            vertices: rkyv::deserialize::<Vec<Vertex>, rkyv::rancor::Error>(
                                &serialized_mesh.vertices,
                            )
                            .unwrap(),
                            indices: rkyv::deserialize::<Vec<u32>, rkyv::rancor::Error>(
                                &serialized_mesh.indices,
                            )
                            .unwrap(),
                        };

                        let mesh_buffer = MeshBuffer {
                            mesh_object_device_address: Default::default(),
                            vertex_buffer_reference,
                            vertex_indices_buffer_reference,
                            meshlets_buffer_reference,
                            local_indices_buffer_reference,
                            meshlets_count: serialized_mesh.meshlets.len(),
                            mesh_data,
                        };

                        let mesh_buffer_reference =
                            mesh_buffers_pool.insert_mesh_buffer(mesh_buffer);

                        mesh_buffers_to_upload
                            .insert(mesh_index.to_native() as usize, mesh_buffer_reference);
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

    fn create_and_copy_to_buffer(
        buffers_pool: &mut BuffersPool,
        src: *const std::ffi::c_void,
        size: usize,
        name: String,
    ) -> BufferReference {
        let buffer_reference = buffers_pool.create_buffer(
            size,
            BufferUsageFlags::TransferDst,
            BufferVisibility::DeviceOnly,
            None,
            Some(name),
        );

        unsafe {
            buffers_pool.transfer_data_to_buffer_raw(buffer_reference, src, size);
        }

        buffer_reference
    }
}

pub fn load_assets_system(
    information: Res<Information>,
    mut loader: ResMut<Loader>,
    mut asset_database: ResMut<AssetDatabase>,
    mut buffers_pool: ResMut<BuffersPool>,
    mut mesh_buffers_pool: ResMut<MeshBuffersPool>,
) {
    let editor_application = information.get_editor_application();

    loader.collect_meta_files(editor_application.get_assets_folder_path());
    loader.resolve_meta_files(
        &mut asset_database,
        information
            .get_editor_application()
            .get_artifacts_folder_path(),
    );
    loader.load_assets(
        &mut asset_database,
        &mut buffers_pool,
        &mut mesh_buffers_pool,
    );
}
