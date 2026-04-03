use connect_information::Information;
use connect_renderer::*;
use connect_shared::*;
use fast_image_resize::{PixelType, images::Image};
use image::{EncodableLayout, ImageReader};
use ktx2_rw::BasisCompressionParams;
use std::{
    collections::{HashMap, HashSet},
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
};
use uuid::{Uuid, uuid};
use walkdir::WalkDir;

use asset_importer::{Matrix4x4, node::Node, postprocess::PostProcessSteps};
use bevy_ecs::{
    resource::Resource,
    system::{Res, ResMut},
};
use connect_math::*;
use meshopt::*;

type ModelLoader = asset_importer::Importer;

struct NodeData {
    pub name: String,
    pub index: usize,
    pub parent_index: Option<usize>,
    pub matrix: Mat4,
    pub mesh_indices: Vec<usize>,
}

impl NodeData {
    pub fn new(
        name: String,
        index: usize,
        parent_index: Option<usize>,
        transformation: Matrix4x4,
        mesh_indices: Vec<usize>,
    ) -> Self {
        let matrix = Self::get_matrix(transformation);

        Self {
            name,
            index,
            parent_index,
            matrix,
            mesh_indices,
        }
    }

    pub fn get_matrix(transformation: Matrix4x4) -> Mat4 {
        connect_math::Mat4 {
            x_axis: Vec4::new(
                transformation.x_axis.x,
                transformation.x_axis.y,
                transformation.x_axis.z,
                transformation.x_axis.w,
            ),
            y_axis: Vec4::new(
                transformation.y_axis.x,
                transformation.y_axis.y,
                transformation.y_axis.z,
                transformation.y_axis.w,
            ),
            z_axis: Vec4::new(
                transformation.z_axis.x,
                transformation.z_axis.y,
                transformation.z_axis.z,
                transformation.z_axis.w,
            ),
            w_axis: Vec4::new(
                transformation.w_axis.x,
                transformation.w_axis.y,
                transformation.w_axis.z,
                transformation.w_axis.w,
            ),
        }
    }
}

pub struct SerializedAssetsPathBuffers {
    pub model_path: PathBuf,
    pub textures_path: PathBuf,
    pub materials_path: PathBuf,
}

#[derive(Resource)]
pub struct Importer {
    model_importer: ModelLoader,
    asset_folder_path_buffer: PathBuf,
    serialized_assets_folder_path_buffer: PathBuf,
    serialized_assets_path_buffers: SerializedAssetsPathBuffers,
    assets_to_serialize: Vec<PathBuf>,
    meta_files: Vec<AssetMetadata>,
    assets_entries: Vec<AssetEntry>,
}

impl Importer {
    const ENGINE_ASSET_NAMESPACE: Uuid = uuid!("7bd2b6c7-4494-4337-a884-6dd216017354");

    pub fn new() -> Self {
        let serialized_assets_folder_path_buffer = Self::get_serialized_assets_folder_path_buffer();

        let model_path = serialized_assets_folder_path_buffer.join("models").clone();
        std::fs::create_dir_all(model_path.as_path()).unwrap();
        let textures_path = serialized_assets_folder_path_buffer
            .join("textures")
            .clone();
        std::fs::create_dir_all(textures_path.as_path()).unwrap();
        let materials_path = serialized_assets_folder_path_buffer
            .join("materials")
            .clone();
        std::fs::create_dir_all(materials_path.as_path()).unwrap();

        Self {
            model_importer: ModelLoader::new(),
            asset_folder_path_buffer: Self::get_assets_folder_path_buffer(),
            serialized_assets_folder_path_buffer: serialized_assets_folder_path_buffer.clone(),
            serialized_assets_path_buffers: SerializedAssetsPathBuffers {
                model_path,
                textures_path,
                materials_path,
            },
            assets_to_serialize: Default::default(),
            meta_files: Vec::new(),
            assets_entries: Vec::new(),
        }
    }

    fn get_assets_folder_path_buffer() -> PathBuf {
        let mut exe_path = std::env::current_exe().unwrap();

        exe_path.pop();
        exe_path.pop();
        exe_path.pop();
        exe_path.push("assets");

        exe_path
    }

    fn get_serialized_assets_folder_path_buffer() -> PathBuf {
        let mut exe_path = std::env::current_exe().unwrap();

        exe_path.pop();
        exe_path.pop();
        exe_path.pop();
        exe_path.push("artifacts");

        exe_path
    }
}

pub fn collect_assets_to_serialize_system(
    mut importer: ResMut<Importer>,
    information: Res<Information>,
) {
    importer.assets_to_serialize.clear();
    importer.meta_files.clear();

    let assets_folder_path = importer.asset_folder_path_buffer.as_path();

    for entry in WalkDir::new(assets_folder_path)
        .into_iter()
        .filter_map(|e| e.ok())
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

                let is_presented_serialized_asset =
                    is_presented_serialized_asset(&information, &meta_file);

                if (is_presented_serialized_asset) {
                    importer.meta_files.push(meta_file);
                } else {
                    importer
                        .assets_to_serialize
                        .push(entry.path().to_path_buf());
                }
            } else {
                importer
                    .assets_to_serialize
                    .push(entry.path().to_path_buf());
            }
        }
    }
}

fn is_presented_serialized_asset(information: &Information, meta_file: &AssetMetadata) -> bool {
    let is_presented_serialized_asset;

    // FIXME: Temp that Material assets are skipped.
    if let AssetMetadata::Material(_) = meta_file {
        is_presented_serialized_asset = true;
    } else {
        // FIXME: Move to the constants folder names in artifacts folder.
        let (serialized_asset_uuid, serialized_asset_name, serialized_asset_folder_name) =
            match meta_file {
                AssetMetadata::Model(model_asset_metadata) => (
                    model_asset_metadata.uuid,
                    model_asset_metadata.name.as_str(),
                    "models",
                ),
                AssetMetadata::Texture(texture_asset_metadata) => (
                    texture_asset_metadata.uuid,
                    texture_asset_metadata.name.as_str(),
                    "textures",
                ),
                AssetMetadata::Material(material_asset_metadata) => (
                    material_asset_metadata.uuid,
                    material_asset_metadata.name.as_str(),
                    "materials",
                ),
            };
        let serialized_asset_uuid_string = serialized_asset_uuid.to_string();
        let artifacts_shard_name = &serialized_asset_uuid_string[0..2];

        let mut serialized_asset_folder_path_buf = information
            .get_editor_application()
            .get_artifacts_folder_path()
            .to_path_buf();
        serialized_asset_folder_path_buf.push(serialized_asset_folder_name);
        serialized_asset_folder_path_buf.push(artifacts_shard_name);
        serialized_asset_folder_path_buf.push(std::format!(
            "{serialized_asset_name}_{serialized_asset_uuid_string}",
        ));

        is_presented_serialized_asset =
            std::fs::exists(serialized_asset_folder_path_buf).unwrap_or(false);
    }

    is_presented_serialized_asset
}

pub fn resolve_assets_entries_system(mut importer: ResMut<Importer>) {
    let mut asset_entries = Vec::with_capacity(importer.assets_to_serialize.len());

    importer
        .assets_to_serialize
        .drain(..)
        .for_each(|asset_to_serialize| {
            let file_name = asset_to_serialize
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned();

            let extension = asset_to_serialize
                .extension()
                .unwrap()
                .to_str()
                .unwrap_or_default();
            match extension {
                // TODO: Currently, we support only "glb" format of models.
                //"glb" | "gltf" | "obj" | "fbx" => {
                "glb" => {
                    asset_entries.push(AssetEntry::Model(ModelEntry {
                        entry: BaseAssetEntry {
                            name: file_name,
                            extension: asset_to_serialize
                                .extension()
                                .unwrap()
                                .to_str()
                                .unwrap()
                                .to_owned(),
                            path_buf: asset_to_serialize.clone(),
                        },
                    }));
                }
                // TODO: Add other formats.
                // TODO: Add hdr image format support for cubemaps.
                "hdr" | "jpg" | "jpeg" | "png" => {
                    let texture_format = match extension {
                        "jpg" | "jpeg" | "png" => TextureFormat::Bc1,
                        "hdr" => TextureFormat::Bc6H,
                        _ => unimplemented!(),
                    };

                    asset_entries.push(AssetEntry::Texture(TextureEntry {
                        entry: BaseAssetEntry {
                            name: file_name,
                            extension: asset_to_serialize
                                .extension()
                                .unwrap()
                                .to_str()
                                .unwrap()
                                .to_owned(),
                            path_buf: asset_to_serialize.clone(),
                        },
                        format: texture_format,
                        associated_model: None,
                    }))
                }
                _ => (),
            }
        });

    importer.assets_entries.clear();
    importer.assets_entries.append(&mut asset_entries);
}

pub fn check_if_asset_is_serialized_system(mut importer: ResMut<Importer>) {
    let meta_files = importer.meta_files.to_vec();
    let asset_folder_path_buffer = importer.asset_folder_path_buffer.clone();

    importer.assets_entries.retain(|asset_entry| {
        let name = match asset_entry {
            AssetEntry::Model(model_entry) => model_entry.entry.name.as_str(),
            AssetEntry::Texture(texture_entry) => texture_entry.entry.name.as_str(),
        };
        let path = match asset_entry {
            AssetEntry::Model(model_entry) => model_entry.entry.path_buf.as_path(),
            AssetEntry::Texture(texture_entry) => texture_entry.entry.path_buf.as_path(),
        };

        !meta_files.iter().any(|meta_file| {
            let meta_name = match meta_file {
                AssetMetadata::Model(model_asset) => model_asset.name.as_str(),
                AssetMetadata::Texture(texture_asset_metadata) => {
                    texture_asset_metadata.name.as_str()
                }
                AssetMetadata::Material(material_asset_metadata) => {
                    material_asset_metadata.name.as_str()
                }
            };
            let meta_path = match meta_file {
                AssetMetadata::Model(model_asset) => model_asset.path_buf.as_path(),
                AssetMetadata::Texture(texture_asset_metadata) => {
                    texture_asset_metadata.path_buf.as_path()
                }
                AssetMetadata::Material(material_asset_metadata) => {
                    material_asset_metadata.path_buf.as_path()
                }
            };

            let path = path
                .strip_prefix(asset_folder_path_buffer.as_path())
                .unwrap();

            name.eq(meta_name) && path.eq(meta_path)
        })
    });
}

pub fn serialize_unserialized_assets_system(mut importer: ResMut<Importer>) {
    // TODO: Handle properly textures assets and materials, for now, we leave it as it is.
    let mut assets_entries = importer.assets_entries.to_vec();

    assets_entries
        .drain(..)
        .for_each(|asset_entry: AssetEntry| match asset_entry {
            AssetEntry::Model(model_entry) => {
                let model_name = model_entry.entry.name.clone();
                let serialized_model_result = serialize_model_asset(&mut importer, &model_entry);
                serialized_model_result
                    .associated_texture_entries
                    .iter()
                    .for_each(|texture_entry| {
                        serialize_texture_asset(&mut importer, texture_entry);
                    });

                let relative_path = model_entry
                    .entry
                    .path_buf
                    .strip_prefix(&importer.asset_folder_path_buffer)
                    .unwrap_or(&model_entry.entry.path_buf)
                    .to_string_lossy();

                let normalized_asset_path = relative_path.replace("\\", "/");

                let uuid = Uuid::new_v5(
                    &Importer::ENGINE_ASSET_NAMESPACE,
                    normalized_asset_path.as_bytes(),
                );
                let uuid_str = uuid.as_simple().to_string();

                let serialized_asset_path = importer
                    .serialized_assets_path_buffers
                    .model_path
                    .join(&uuid_str[0..2]);
                std::fs::create_dir_all(serialized_asset_path.as_path()).unwrap();

                let serialized_model_path_buffer = serialized_asset_path
                    .join(std::format!("{}_{}", model_name, uuid))
                    .clone();
                let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(
                    &serialized_model_result.serialized_model,
                )
                .expect("Failed to serialize model.");

                std::fs::write(serialized_model_path_buffer, bytes).unwrap();

                let model_asset_metadata = AssetMetadata::Model(ModelAssetMetadata {
                    uuid,
                    name: model_name,
                    path_buf: PathBuf::from(normalized_asset_path),
                    // TODO: Temp commenting.
                    // textures,
                });
                let serialized_model_asset_metadata =
                    toml::ser::to_string_pretty(&model_asset_metadata).unwrap();

                let model_asset_metadata_path = model_entry.entry.path_buf.clone();

                std::fs::write(
                    std::format!("{}.meta", model_asset_metadata_path.display()),
                    serialized_model_asset_metadata,
                )
                .unwrap();
            }
            AssetEntry::Texture(texture_entry) => {}
        });
}

// TODO: Currently, we serialize and model, and textures, and materials in the same pass, later, need to separate them.
fn serialize_model_asset(
    importer: &mut Importer,
    model_entry: &ModelEntry,
) -> SerializedModelResult {
    let model_path = model_entry.entry.path_buf.as_path();
    let model_name = model_entry.entry.name.split('.').next().unwrap();

    let scene = importer
        .model_importer
        .read_file(model_path)
        .with_post_process(PostProcessSteps::MAX_QUALITY | PostProcessSteps::FLIP_UVS)
        .import()
        .unwrap();

    let mut nodes = Vec::new();

    let root_node_index = Default::default();
    let root_node = scene.root_node().unwrap();

    nodes.push(NodeData::new(
        root_node.name(),
        root_node_index,
        None,
        root_node.transformation(),
        get_mesh_indices(&root_node, root_node.num_meshes()),
    ));

    let mut stack: Vec<(Node, usize)> = Vec::new();
    stack.push((root_node, root_node_index));

    loop {
        while let Some((parent_node, parent_index_in_array)) = stack.pop() {
            for child_index in (0..parent_node.num_children()).rev() {
                let child_node = parent_node.child(child_index).unwrap();

                let child_index_in_array = nodes.len();
                stack.push((child_node.clone(), child_index_in_array));

                nodes.push(NodeData::new(
                    child_node.name(),
                    child_index_in_array,
                    Some(parent_index_in_array),
                    child_node.transformation(),
                    get_mesh_indices(&child_node, child_node.num_meshes()),
                ));
            }
        }

        if stack.len() == Default::default() {
            break;
        }
    }

    let mut serialized_model = SerializedModel {
        meshes: Vec::new(),
        hierarchy: SerializedHierarchy {
            serialized_nodes: Vec::new(),
        },
    };

    nodes.iter().for_each(|node_data| {
        let local_matrix = node_data.matrix;

        let serialized_node = SerializedNode {
            name: node_data.name.clone(),
            index: node_data.index,
            parent_index: node_data.parent_index,
            matrix: local_matrix.to_cols_array(),
            mesh_index: None,
        };

        serialized_model
            .hierarchy
            .serialized_nodes
            .push(serialized_node);
    });

    let mut serialized_meshes = HashMap::with_capacity(scene.num_meshes());
    // TODO: Temp.
    let mut extracted_textures = HashSet::with_capacity(scene.textures().count());
    //let mut extracted_materials = HashSet::with_capacity(scene.textures().count());
    let mut associated_texture_entries: Vec<TextureEntry> =
        Vec::with_capacity(scene.textures().count());
    //let mut serialized_textures = HashMap::with_capacity(serialized_meshes.capacity());
    /*let mut uploaded_materials = HashMap::with_capacity(scene.num_materials()) */

    for node_data in nodes.into_iter() {
        if node_data.mesh_indices.len() > Default::default() {
            let mut mesh_name: String;

            let mut texture_index: Option<u32>;
            let mut mesh_index: usize;

            for &current_mesh_index in node_data.mesh_indices.iter() {
                if let std::collections::hash_map::Entry::Vacant(entry) =
                    serialized_meshes.entry(current_mesh_index)
                {
                    let mesh = scene.mesh(current_mesh_index).unwrap();
                    mesh_name = mesh.name();

                    let mut indices = Vec::with_capacity(mesh.faces().len() * 3);

                    for face in mesh.faces() {
                        for index in face.indices() {
                            indices.push(*index);
                        }
                    }

                    let positions: Vec<Vec3> = mesh
                        .vertices_iter()
                        .map(|v| Vec3::new(v.x, v.y, v.z))
                        .collect();
                    let colors: Vec<Vec3> = mesh
                        .vertex_colors(Default::default())
                        .map(|colors| {
                            colors
                                .iter()
                                .map(|color| Vec3::new(color.x, color.y, color.z))
                                .collect()
                        })
                        .unwrap_or_else(|| vec![Vec3::ZERO; positions.len()]);
                    let normals: Vec<Vec3> = mesh
                        .normals()
                        .map(|ns| ns.iter().map(|n| Vec3::new(n.x, n.y, n.z)).collect())
                        .unwrap_or_else(|| vec![Vec3::ZERO; positions.len()]);

                    let uvs: Vec<Vec2> = if mesh.has_texture_coords(0) {
                        mesh.texture_coords_iter(0)
                            .map(|uv| Vec2::new(uv.x, uv.y))
                            .collect()
                    } else {
                        vec![Vec2::ZERO; positions.len()]
                    };

                    let mut vertices = Vec::with_capacity(positions.len());
                    for i in 0..positions.len() {
                        vertices.push(connect_shared::Vertex {
                            position: positions[i].to_array(),
                            normal: normals[i].to_array(),
                            uv: uvs[i].to_array(),
                            color: colors[i].to_array(),
                            ..Default::default()
                        });
                    }

                    let remap = optimize_vertex_fetch_remap(&indices, vertices.len());
                    indices = remap_index_buffer(Some(&indices), vertices.len(), &remap);
                    vertices = remap_vertex_buffer(&vertices, vertices.len(), &remap);

                    let position_offset = std::mem::offset_of!(connect_shared::Vertex, position);
                    let vertex_stride = std::mem::size_of::<connect_shared::Vertex>();

                    // TODO: Use bytemuck instead.
                    let vertex_data = typed_to_bytes(&vertices);

                    let vertex_data_adapter =
                        VertexDataAdapter::new(vertex_data, vertex_stride, position_offset)
                            .unwrap();

                    optimize_vertex_cache_in_place(&mut indices, vertices.len());
                    let vertices = optimize_vertex_fetch(&mut indices, &vertices);

                    let (meshlets, vertex_indices, triangles) =
                        generate_meshlets(&indices, &vertex_data_adapter);

                    let mut serialized_mesh = SerializedMesh {
                        name: mesh.name(),
                        vertices,
                        indices: vertex_indices,
                        meshlets,
                        triangles,
                        material_uuid: Uuid::nil(),
                    };

                    mesh_index = serialized_model.meshes.len();

                    let material_index = mesh.material_index();

                    let material = scene.material(material_index).unwrap();
                    let texture_base_asset_entry = extract_texture_from_material(
                        &scene,
                        &model_name,
                        PathBuf::from(model_path),
                        &mut extracted_textures,
                        material.clone(),
                    );

                    let mut texture_format = TextureFormat::Bc1;

                    let is_material_transparent = is_material_transparent(&material);
                    // TODO: In future, texture format isn't dependent by material type. Type and texture format are independent (Material Type != Texture Format).
                    if is_material_transparent {
                        texture_format = TextureFormat::Bc3;
                    }

                    let mut textures_assets_metadata = Vec::new();
                    if let Some(asset_entry) = texture_base_asset_entry {
                        let texture_entry = TextureEntry {
                            entry: asset_entry.clone(),
                            format: texture_format,
                            associated_model: Some(model_entry.clone()),
                        };

                        let texture_asset_metadata =
                            serialize_texture_asset(importer, &texture_entry);
                        textures_assets_metadata.push(texture_asset_metadata);
                        //associated_texture_entries.push(texture_entry);
                    }

                    // MATERIAL //////////////////////////////////////////
                    let mut material_type = MaterialType::Opaque;
                    if is_material_transparent {
                        material_type = MaterialType::Transparent;
                    }

                    let base_color_raw = material.base_color().unwrap();
                    let base_color = Vec4::new(
                        base_color_raw.x,
                        base_color_raw.y,
                        base_color_raw.z,
                        base_color_raw.w,
                    );

                    let metallic_value = material.metallic_factor().unwrap_or(0.0);
                    let roughness_value = material.roughness_factor().unwrap_or(0.0);
                    let albedo_texture_index = u32::default();
                    let metallic_texture_index = u32::default();
                    let roughness_texture_index = u32::default();

                    let material_data = MaterialData {
                        material_properties: MaterialProperties::new(
                            base_color,
                            metallic_value,
                            roughness_value,
                        ),
                        material_textures: MaterialTextures::new(
                            albedo_texture_index,
                            metallic_texture_index,
                            roughness_texture_index,
                        ),
                        sampler_index: Default::default(),
                    };
                    let material_data_raw = bytemuck::bytes_of(&material_data);
                    let material_uuid = serialize_material(
                        importer,
                        PathBuf::from(model_path),
                        material_data_raw,
                        model_name,
                        &material.name(),
                        textures_assets_metadata,
                    );

                    // TODO: Move to the initialization place.
                    serialized_mesh.material_uuid = material_uuid;

                    /////////////////////////////////////////////////////////////

                    entry.insert((mesh, mesh_index));
                    serialized_model.meshes.push(serialized_mesh);
                } else {
                    let already_uploaded_mesh = serialized_meshes.get(&current_mesh_index).unwrap();
                    mesh_name = already_uploaded_mesh.0.name();
                    mesh_index = already_uploaded_mesh.1;
                }

                let serialized_node = SerializedNode {
                    name: mesh_name,
                    index: node_data.index,
                    parent_index: Some(node_data.index),
                    matrix: node_data.matrix.to_cols_array(),
                    mesh_index: Some(mesh_index),
                };

                serialized_model
                    .hierarchy
                    .serialized_nodes
                    .push(serialized_node);
            }
        }
    }

    SerializedModelResult {
        serialized_model,
        associated_texture_entries,
    }
}

// TODO: Handle, when texture is not part of model's binary.
fn extract_texture_from_material(
    scene: &asset_importer::Scene,
    model_name: &str,
    mut model_path: PathBuf,
    extracted_textures: &mut HashSet<usize>,
    material: asset_importer::Material,
) -> Option<BaseAssetEntry> {
    let mut base_asset_entry: Option<BaseAssetEntry> = None;

    if material.texture_count(asset_importer::TextureType::BaseColor) > Default::default() {
        let texture_info = material
            .texture(asset_importer::TextureType::BaseColor, Default::default())
            .unwrap();
        let texture_index = texture_info.path[1..].parse::<usize>().unwrap();

        if !extracted_textures.contains(&texture_index) {
            let texture = scene.texture(texture_index).unwrap();
            let format = texture.format_hint();
            let texture_name = texture
                .filename()
                .unwrap_or(std::format!("{model_name}_texture_{texture_index}"));

            model_path.pop();
            let target_path = PathBuf::from(model_path)
                .join(std::format!("{}_media", model_name))
                .join("textures");
            std::fs::create_dir_all(&target_path).unwrap();

            // TODO: Currently, we upload only base maps, so, we're hardcoding prefix of texture.
            let texture_path = target_path.clone().join(std::format!(
                "base_{}_{}.{}",
                model_name,
                texture_name,
                format
            ));

            let mut texture_file: std::fs::File =
                std::fs::File::create(texture_path.as_path()).unwrap();
            let data = texture.data_bytes_ref().unwrap();
            texture_file.write(&data).unwrap();

            base_asset_entry = Some(BaseAssetEntry {
                name: texture_name,
                extension: format,
                path_buf: texture_path,
            });
        }
    }

    base_asset_entry
}

fn serialize_material(
    importer: &Importer,
    mut model_path: PathBuf,
    material_data: &[u8],
    model_name: &str,
    material_name: &str,
    mut associated_textures_assets_metadata: Vec<TextureAssetMetadata>,
) -> Uuid {
    model_path.pop();
    let target_path = model_path
        .join(std::format!("{}_media", model_name))
        .join("materials");
    std::fs::create_dir_all(&target_path).unwrap();

    let serialized_material_path_buffer = target_path
        .join(std::format!("{}_{}.mat", model_name, material_name))
        .clone();
    let normalized_asset_path = serialized_material_path_buffer
        .strip_prefix(&importer.asset_folder_path_buffer)
        .unwrap();

    let uuid_name = std::format!("{}{}", normalized_asset_path.display(), material_name);
    let uuid = Uuid::new_v5(&Importer::ENGINE_ASSET_NAMESPACE, uuid_name.as_bytes());

    let texture_inputs = associated_textures_assets_metadata
        .iter()
        .enumerate()
        .map(|(index, associated_texture_asset_metadata)| TextureInput {
            uuid: associated_texture_asset_metadata.uuid,
            offset: std::mem::offset_of!(MaterialData, material_textures)
                + std::mem::offset_of!(MaterialTextures, albedo_texture_index),
            ..Default::default()
        })
        .collect();

    let serialized_material = SerializedMaterial {
        size: material_data.len(),
        data: material_data.to_vec(),
        texture_inputs,
        ..Default::default()
    };
    let serialized_material_raw =
        rkyv::to_bytes::<rkyv::rancor::Error>(&serialized_material).unwrap();

    std::fs::write(
        serialized_material_path_buffer.as_path(),
        serialized_material_raw,
    )
    .unwrap();

    let textures = associated_textures_assets_metadata
        .drain(..)
        .map(|associated_texture_asset_metadata| associated_texture_asset_metadata.uuid)
        .collect();

    let material_asset_metadata = AssetMetadata::Material(MaterialAssetMetadata {
        uuid,
        name: material_name.to_string(),
        path_buf: normalized_asset_path.to_path_buf(),
        textures,
        // TODO: Temp commenting.
        // textures,
    });
    let serialized_texture_asset_metadata =
        toml::ser::to_string_pretty(&material_asset_metadata).unwrap();

    std::fs::write(
        std::format!("{}.meta", serialized_material_path_buffer.display()),
        serialized_texture_asset_metadata,
    )
    .unwrap();

    uuid
}

fn serialize_texture_asset(
    importer: &mut Importer,
    texture_entry: &TextureEntry,
) -> TextureAssetMetadata {
    let mut texture_file = std::fs::File::open(texture_entry.entry.path_buf.as_path()).unwrap();
    let mut data = Vec::new();
    texture_file.read_to_end(&mut data).unwrap();

    let cursor = Cursor::new(&mut data);

    let image = ImageReader::new(cursor)
        .with_guessed_format()
        .unwrap()
        .decode()
        .unwrap();

    let width = image.width();
    let height = image.height();

    let rgba_image = image.to_rgba8();
    let mut image_bytes = rgba_image.as_bytes().to_vec();

    // TODO: Assume that mip-map enabled by default.
    let mip_map_enabled = true;

    let mip_levels_count = if mip_map_enabled {
        f32::max(width as _, height as _).log2().floor() as u32 + 1
    } else {
        1
    };

    let target_ktx_format = match texture_entry.format {
        TextureFormat::Bc1 | TextureFormat::Bc3 => ktx2_rw::VkFormat::R8G8B8A8_SRGB,
        _ => panic!("Unsupported KTX format: {:?}!", texture_entry.format),
    };

    let mut ktx_texture =
        ktx2_rw::Ktx2Texture::create(width, height, 1, 1, 1, mip_levels_count, target_ktx_format)
            .unwrap();

    let src_image = match texture_entry.format {
        TextureFormat::Bc3 => {
            Image::from_slice_u8(width, height, &mut image_bytes, PixelType::U8x4).unwrap()
        }
        TextureFormat::Bc1 => {
            Image::from_slice_u8(width, height, &mut image_bytes, PixelType::U8x4).unwrap()
        }
        _ => panic!("Unsupported Image format: {:?}!", texture_entry.format),
    };

    // TODO: We can effectively pre-allocate required total size of texture_data
    let mut texture_data = Vec::new();
    for mip_level_index in 0..mip_levels_count {
        let current_width = (width >> mip_level_index).max(1);
        let current_height = (height >> mip_level_index).max(1);

        let mut resizer = fast_image_resize::Resizer::new();
        unsafe {
            resizer.set_cpu_extensions(fast_image_resize::CpuExtensions::Avx2);
        }

        let mut dst_image = fast_image_resize::images::Image::new(
            current_width,
            current_height,
            src_image.pixel_type(),
        );

        resizer.resize(&src_image, &mut dst_image, None).unwrap();

        let image_bytes = dst_image.buffer();

        ktx_texture
            .set_image_data(mip_level_index, 0, 0, image_bytes)
            .unwrap();
    }

    ktx_texture
        .compress_basis(
            &BasisCompressionParams::builder()
                .thread_count((num_cpus::get() - 1) as _)
                .build(),
        )
        .unwrap();

    let transcode_format = match texture_entry.format {
        TextureFormat::Bc1 => ktx2_rw::TranscodeFormat::Bc1Rgb,
        TextureFormat::Bc3 => ktx2_rw::TranscodeFormat::Bc3Rgba,
        TextureFormat::Bc7 => ktx2_rw::TranscodeFormat::Bc7Rgba,
        _ => panic!("Unsupported transcode format!"),
    };

    ktx_texture.transcode_basis(transcode_format).unwrap();

    for mip_level_index in 0..mip_levels_count {
        let texture_data_ref = ktx_texture.get_image_data(mip_level_index, 0, 0).unwrap();
        texture_data.extend_from_slice(texture_data_ref);
    }

    let texture_metadata = TextureMetadata {
        texture_format: texture_entry.format,
        width,
        height,
        mip_levels_count,
        ..Default::default()
    };
    let texture_metadata_raw = &rkyv::to_bytes::<rkyv::rancor::Error>(&texture_metadata).unwrap();

    ktx_texture
        .set_metadata(
            stringify!(TextureMetadata),
            &texture_metadata_raw.as_bytes(),
        )
        .unwrap();

    for mip_level_index in 0..mip_levels_count {
        texture_data.extend_from_slice(ktx_texture.get_image_data(mip_level_index, 0, 0).unwrap());
    }

    let relative_path = texture_entry
        .entry
        .path_buf
        .strip_prefix(&importer.asset_folder_path_buffer)
        .unwrap_or(&texture_entry.entry.path_buf)
        .to_string_lossy();

    let normalized_asset_path = relative_path.replace("\\", "/");

    let uuid = Uuid::new_v5(
        &Importer::ENGINE_ASSET_NAMESPACE,
        normalized_asset_path.as_bytes(),
    );
    let uuid_str = uuid.as_simple().to_string();

    let serialized_asset_path = importer
        .serialized_assets_path_buffers
        .textures_path
        .join(&uuid_str[0..2]);
    std::fs::create_dir_all(serialized_asset_path.as_path()).unwrap();

    let serialized_texture_path_buffer = serialized_asset_path
        .join(std::format!("{}_{}", texture_entry.entry.name, uuid))
        .clone();

    ktx_texture
        .write_to_file(serialized_texture_path_buffer.as_path())
        .unwrap();
    let texture_asset_metadata = TextureAssetMetadata {
        uuid,
        name: texture_entry.entry.name.clone(),
        path_buf: PathBuf::from(normalized_asset_path),
        // TODO: Temp commenting.
        // textures,
    };

    let asset_metadata = AssetMetadata::Texture(texture_asset_metadata.clone());
    let serialized_texture_asset_metadata = toml::ser::to_string_pretty(&asset_metadata).unwrap();

    let texture_asset_metadata_path = texture_entry.entry.path_buf.clone();

    std::fs::write(
        std::format!("{}.meta", texture_asset_metadata_path.display()),
        serialized_texture_asset_metadata,
    )
    .unwrap();

    texture_asset_metadata
}

fn is_material_transparent(material: &asset_importer::Material) -> bool {
    let alpha_mode = std::str::from_utf8(
        material
            .get_property_raw_ref(c"$mat.gltf.alphaMode", None, 0)
            .unwrap(),
    )
    .unwrap();

    alpha_mode.contains("BLEND")
}

fn get_mesh_indices(node: &Node, num_meshes: usize) -> Vec<usize> {
    let mut mesh_indices = Vec::with_capacity(num_meshes);
    if num_meshes > Default::default() {
        for mesh_index in node.mesh_indices() {
            mesh_indices.push(mesh_index);
        }
    }

    mesh_indices
}

fn generate_meshlets(
    indices: &[u32],
    vertices: &VertexDataAdapter,
) -> (Vec<connect_shared::Meshlet>, Vec<u32>, Vec<u8>) {
    let max_vertices = 64;
    let max_triangles = 64;
    let cone_weight = 0.0;

    let raw_meshlets = build_meshlets(indices, vertices, max_vertices, max_triangles, cone_weight);

    let mut meshlets = Vec::with_capacity(raw_meshlets.len());

    for raw_meshlet in raw_meshlets.meshlets.iter() {
        meshlets.push(connect_shared::Meshlet {
            vertex_offset: raw_meshlet.vertex_offset as _,
            triangle_offset: raw_meshlet.triangle_offset as _,
            vertex_count: raw_meshlet.vertex_count as _,
            triangle_count: raw_meshlet.triangle_count as _,
            ..Default::default()
        });
    }

    (meshlets, raw_meshlets.vertices, raw_meshlets.triangles)
}
