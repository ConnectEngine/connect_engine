use std::{
    collections::HashMap,
    io::Read,
    path::{Path, PathBuf},
};

use bevy_ecs::{
    resource::Resource,
    system::{Res, ResMut},
};
use connect_asset_database::AssetDatabase;
use connect_information::Information;
use connect_math::*;
use connect_shared::*;
use memmap2::Mmap;
use uuid::Uuid;
use vulkanite::vk::{
    BufferCopy, BufferUsageFlags, DeviceAddress, Format, ImageAspectFlags, ImageUsageFlags,
};
use walkdir::WalkDir;

mod events;

pub use events::*;

use connect_renderer::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum AssetType {
    Model,
    Texture,
    Material,
}

struct AssetToLoad {
    pub uuid: Uuid,
    pub name: String,
    pub path_buf: PathBuf,
    pub original_path_buf: PathBuf,
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
        assets_folder_path: &Path,
        artifacts_folder_path: &Path,
    ) {
        self.collected_meta_files
            .drain(..)
            .for_each(|meta_file| match meta_file {
                AssetMetadata::Model(model_asset_metadata) => {
                    self.models_to_load.push(AssetToLoad {
                        uuid: model_asset_metadata.uuid,
                        name: model_asset_metadata.name.clone(),
                        original_path_buf: model_asset_metadata.path_buf,
                        path_buf: Self::resolve_path(
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
                        original_path_buf: texture_asset_metadata.path_buf,
                        path_buf: Self::resolve_path(
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
                        original_path_buf: material_asset_metadata.path_buf.clone(),
                        path_buf: assets_folder_path.join(material_asset_metadata.path_buf),
                    });
                }
            });
    }

    pub(crate) fn load_assets(
        &mut self,
        asset_database: &mut AssetDatabase,
        vulkan_context_resource: &VulkanContextResource,
        renderer_context_resource: &RendererContextResource,
        descriptor_set_handle: &mut DescriptorSetHandle,
        buffers_pool: &mut BuffersPool,
        textures_pool: &mut TexturesPoolResource,
        mesh_buffers_pool: &mut MeshBuffersPool,
    ) {
        self.models_to_load.iter().for_each(|model_to_load| {
            let serialized_model_file = std::fs::File::open(&model_to_load.path_buf).unwrap();

            let serialized_model_map = unsafe { Mmap::map(&serialized_model_file).unwrap() };

            let archived_serialized_model =
                rkyv::access::<ArchivedSerializedModel, rkyv::rancor::Error>(&serialized_model_map)
                    .unwrap();

            let mut spawn_event = SpawnEvent::default();
            spawn_event.parent_entity = None;
            let mut spawn_event_record = SpawnEventRecord::default();

            archived_serialized_model
                .hierarchy
                .serialized_nodes
                .iter()
                .for_each(|node| {
                    let local_matrix = Mat4::from_cols_array(&node.matrix.map(|f| f.to_native()));

                    let (local_scale, rotation, position) =
                        local_matrix.to_scale_rotation_translation();
                    let transform = LocalTransform {
                        local_position: position,
                        local_rotation: rotation,
                        local_scale,
                    };
                    spawn_event_record.name = node.name.to_string();
                    spawn_event_record.parent_index = node
                        .parent_index
                        .as_ref()
                        .map(|&parent_index_value| parent_index_value.to_native() as usize);
                    spawn_event_record.transform = transform;

                    spawn_event.spawn_records.push(spawn_event_record.clone());
                });

            let mut mesh_buffers_to_upload =
                Vec::with_capacity(archived_serialized_model.meshes.len());
            let mut uploaded_meshes =
                HashMap::with_capacity(archived_serialized_model.meshes.len());

            archived_serialized_model
                .hierarchy
                .serialized_nodes
                .iter()
                .for_each(|node| {
                    if node.mesh_index.is_some() {
                        let mesh_index = node.mesh_index.unwrap().to_native() as usize;

                        let mesh_buffer_reference: MeshBufferReference;
                        let mesh_name: String;
                        if let std::collections::hash_map::Entry::Vacant(e) =
                            uploaded_meshes.entry(mesh_index)
                        {
                            let serialized_mesh = archived_serialized_model
                                .meshes
                                .get(mesh_index as usize)
                                .unwrap();

                            // TODO: Handle Material == None
                            self.load_material(
                                asset_database,
                                vulkan_context_resource,
                                renderer_context_resource,
                                descriptor_set_handle,
                                textures_pool,
                                buffers_pool,
                                serialized_mesh.material_uuid,
                            );

                            mesh_name = serialized_mesh.name.to_string();

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
                                std::format!(
                                    "{}_{}",
                                    serialized_mesh.name,
                                    stringify!(vertex_indices)
                                ),
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

                            mesh_buffer_reference =
                                mesh_buffers_pool.insert_mesh_buffer(mesh_buffer);

                            mesh_buffers_to_upload.push(mesh_buffer_reference);

                            e.insert((mesh_name.clone(), mesh_buffer_reference));
                        } else {
                            let uploaded_mesh = uploaded_meshes.get(&mesh_index).unwrap();
                            mesh_name = uploaded_mesh.0.clone();
                            mesh_buffer_reference = uploaded_mesh.1;
                        }

                        spawn_event_record.name = mesh_name;
                        spawn_event_record.parent_index = node
                            .parent_index
                            .as_ref()
                            .map(|parent_index_value| parent_index_value.to_native() as usize);
                        // FIXME
                        //spawn_event_record.material_reference = Some(material_reference);
                        spawn_event_record.material_reference = None;
                        spawn_event_record.mesh_buffer_reference = Some(mesh_buffer_reference);
                        spawn_event_record.transform = LocalTransform::IDENTITY;

                        spawn_event.spawn_records.push(spawn_event_record.clone());
                    }
                });

            let mesh_objects_to_write = mesh_buffers_to_upload
                .iter()
                .map(|mesh_buffer_reference| {
                    let mesh_buffer_ref = unsafe {
                        mesh_buffers_pool
                            .get_mesh_buffer(*mesh_buffer_reference)
                            .unwrap_unchecked()
                    };

                    let device_address_vertex_buffer: DeviceAddress = mesh_buffer_ref
                        .vertex_buffer_reference
                        .get_buffer_info()
                        .device_address;
                    let device_address_vertex_indices_buffer: DeviceAddress = mesh_buffer_ref
                        .vertex_indices_buffer_reference
                        .get_buffer_info()
                        .device_address;
                    let device_address_meshlets_buffer: DeviceAddress = mesh_buffer_ref
                        .meshlets_buffer_reference
                        .get_buffer_info()
                        .device_address;
                    let device_address_local_indices_buffer: DeviceAddress = mesh_buffer_ref
                        .local_indices_buffer_reference
                        .get_buffer_info()
                        .device_address;

                    MeshObject {
                        device_address_vertex_buffer,
                        device_address_vertex_indices_buffer,
                        device_address_meshlets_buffer,
                        device_address_local_indices_buffer,
                        ..Default::default()
                    }
                })
                .collect::<Vec<_>>();

            let mesh_object_size = std::mem::size_of::<MeshObject>();
            let mesh_objects_device_address = mesh_buffers_pool
                .get_mesh_objects_buffer_reference()
                .get_buffer_info()
                .device_address;

            let mesh_objects_to_copy_regions = mesh_buffers_to_upload
                .into_iter()
                .enumerate()
                .map(|(src_mesh_buffer_index, mesh_buffer_reference)| {
                    let src_offset = src_mesh_buffer_index as u32 * mesh_object_size as u32;
                    let dst_offset = mesh_buffer_reference.get_index() * mesh_object_size as u32;

                    let mesh_buffer = unsafe {
                        mesh_buffers_pool
                            .get_mesh_buffer_mut(mesh_buffer_reference)
                            .unwrap_unchecked()
                    };

                    mesh_buffer.mesh_object_device_address =
                        mesh_objects_device_address + dst_offset as u64;

                    BufferCopy {
                        src_offset: src_offset as _,
                        dst_offset: dst_offset as _,
                        size: mesh_object_size as _,
                    }
                })
                .collect::<Vec<BufferCopy>>();

            unsafe {
                buffers_pool.transfer_data_to_buffer_with_offset(
                    mesh_buffers_pool.get_mesh_objects_buffer_reference(),
                    mesh_objects_to_write.as_ptr() as *const _,
                    &mesh_objects_to_copy_regions,
                );
            }
        });
    }

    fn load_material(
        &self,
        asset_database: &mut AssetDatabase,
        vulkan_context_resource: &VulkanContextResource,
        renderer_context_resource: &RendererContextResource,
        descriptor_set_handle: &mut DescriptorSetHandle,
        textures_pool: &mut TexturesPoolResource,
        buffers_pool: &mut BuffersPool,
        material_uuid: Uuid,
    ) {
        let found_material = self
            .materials_to_load
            .iter()
            .find(|material_asset| material_asset.uuid == material_uuid)
            .expect(&std::format!(
                "Cannot find material asset with UUID: {}",
                material_uuid
            ));

        let serialized_material_file = std::fs::File::open(&found_material.path_buf).unwrap();
        let serialized_material_map = unsafe { Mmap::map(&serialized_material_file).unwrap() };
        let archived_serialized_material = rkyv::access::<
            ArchivedSerializedMaterial,
            rkyv::rancor::Error,
        >(&serialized_material_map)
        .unwrap();

        archived_serialized_material
            .texture_inputs
            .iter()
            .for_each(|texture_input| {
                let target_texture_uuid = texture_input.uuid;

                self.load_texture(
                    asset_database,
                    vulkan_context_resource,
                    renderer_context_resource,
                    descriptor_set_handle,
                    textures_pool,
                    buffers_pool,
                    target_texture_uuid,
                );
            });
    }

    fn load_texture(
        &self,
        assset_database: &mut AssetDatabase,
        vulkan_context_resource: &VulkanContextResource,
        renderer_context_resource: &RendererContextResource,
        descriptor_set_handle: &mut DescriptorSetHandle,
        textures_pool: &mut TexturesPoolResource,
        buffers_pool: &mut BuffersPool,
        texture_uuid: Uuid,
    ) {
        let found_texture = self
            .textures_to_load
            .iter()
            .find(|texture_to_load| texture_to_load.uuid == texture_uuid)
            .expect(&std::format!(
                "Cannot find texture asset with UUID: {}",
                texture_uuid
            ));

        let serialized_texture_file = std::fs::File::open(&found_texture.path_buf).unwrap();
        let serialized_texture_map = unsafe { Mmap::map(&serialized_texture_file).unwrap() };

        let ktx_texture = ktx2_rw::Ktx2Texture::from_memory(&serialized_texture_map).unwrap();
        let texture_metadata_raw = ktx_texture
            .get_metadata(stringify!(TextureMetadata))
            .unwrap();

        let archived_texture_metadata =
            rkyv::access::<ArchivedTextureMetadata, rkyv::rancor::Error>(&texture_metadata_raw)
                .unwrap();
        let texture_metadata =
            rkyv::deserialize::<TextureMetadata, rkyv::rancor::Error>(archived_texture_metadata)
                .unwrap();

        let texture_reference = self.upload_texture(
            vulkan_context_resource,
            renderer_context_resource,
            descriptor_set_handle,
            textures_pool,
            buffers_pool,
            &texture_metadata,
            &serialized_texture_map,
        );

        assset_database.track_texture(texture_reference, found_texture.original_path_buf.clone());
    }

    pub fn upload_texture(
        &self,
        vulkan_context_resource: &VulkanContextResource,
        renderer_context_resource: &RendererContextResource,
        descriptor_set_handle: &mut DescriptorSetHandle,
        textures_pool: &mut TexturesPoolResource,
        buffers_pool: &mut BuffersPool,
        texture_metadata: &TextureMetadata,
        data: &[u8],
    ) -> TextureReference {
        let texture_format: Format = texture_metadata.texture_format.try_into().unwrap();
        let texture_reference = textures_pool.upload_texture(
            texture_format,
            vulkanite::vk::Extent3D {
                width: texture_metadata.width,
                height: texture_metadata.height,
                depth: 1,
            },
            ImageUsageFlags::Sampled | ImageUsageFlags::TransferDst,
            texture_metadata.mip_levels_count,
            ImageAspectFlags::Color,
            true,
        );

        vulkan_context_resource.transfer_data_to_image(
            textures_pool.get_image(texture_reference).unwrap(),
            buffers_pool,
            data.as_ptr() as *const _,
            &renderer_context_resource.upload_context,
            Some(data.len()),
        );

        let descriptor_texture = DescriptorKind::SampledImage(DescriptorSampledImage {
            image_view: textures_pool
                .get_image(texture_reference)
                .unwrap()
                .image_view,
            index: texture_reference.get_index(),
        });
        descriptor_set_handle.update_binding(buffers_pool, descriptor_texture);

        let texture_metadata = texture_reference.texture_metadata;
        println!(
            "Name: {} | Index: {} | Extent: {}x{}x{}",
            "FIXME: TEXTURE METADATA DOESN'T HAVE FIELD 'Name'",
            texture_reference.get_index(),
            texture_metadata.width,
            texture_metadata.height,
            1,
        );

        texture_reference
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
    vulkan_context_resource: Res<VulkanContextResource>,
    renderer_context_resource: Res<RendererContextResource>,
    mut descriptor_set_handle: ResMut<DescriptorSetHandle>,
    mut buffers_pool: ResMut<BuffersPool>,
    mut textures_pool: ResMut<TexturesPoolResource>,
    mut mesh_buffers_pool: ResMut<MeshBuffersPool>,
) {
    let editor_application = information.get_editor_application();

    loader.collect_meta_files(editor_application.get_assets_folder_path());
    loader.resolve_meta_files(
        &mut asset_database,
        information
            .get_editor_application()
            .get_assets_folder_path(),
        information
            .get_editor_application()
            .get_artifacts_folder_path(),
    );
    loader.load_assets(
        &mut asset_database,
        &vulkan_context_resource,
        &renderer_context_resource,
        &mut descriptor_set_handle,
        &mut buffers_pool,
        &mut textures_pool,
        &mut mesh_buffers_pool,
    );
}
