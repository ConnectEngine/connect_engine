use fast_image_resize::{PixelType, images::Image};
use image::{EncodableLayout, ImageReader};
use ktx2_rw::BasisCompressionParams;
use std::{
    collections::{HashMap, HashSet},
    default,
    io::{Cursor, Read, Write},
    path::PathBuf,
};
use uuid::{Uuid, uuid};

use asset_importer::{Matrix4x4, node::Node, postprocess::PostProcessSteps};
use bevy_ecs::{resource::Resource, system::ResMut};
use bytemuck::{Pod, Zeroable};
use math::*;
use meshopt::*;
use padding_struct::padding_struct;
use walkdir::WalkDir;

type ModelLoader = asset_importer::Importer;

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct SerializedMesh {
    // NOTE: Vertices and Inddices baked by meshopt, can be issues with creating colliders, but need to check.
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub meshlets: Vec<Meshlet>,
    pub triangles: Vec<u8>,
}

#[repr(C)]
#[derive(Default, Clone, Copy, Pod, Zeroable)]
pub struct TextureMetadata {
    pub texture_format: u32,
    pub width: u32,
    pub height: u32,
    pub mip_levels_count: u32,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct SerializedHierarchy {
    pub serialized_nodes: Vec<SerializedNode>,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct SerializedNode {
    pub name: String,
    pub parent_index: Option<usize>,
    pub matrix: [f32; 16],
    pub mesh_index: Option<usize>,
}

pub struct SerializedModeResult {
    pub serialized_model: SerializedModel,
    pub associated_texture_entries: Vec<TextureEntry>,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct SerializedModel {
    pub meshes: Vec<SerializedMesh>,
    pub hierarchy: SerializedHierarchy,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct SerializedTexture {
    pub data: Vec<u8>,
}

#[repr(C)]
#[padding_struct]
#[derive(
    Default, Clone, Copy, Pod, Zeroable, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize,
)]
pub struct Meshlet {
    pub vertex_offset: u32,
    pub triangle_offset: u32,
    pub vertex_count: u32,
    pub triangle_count: u32,
}

#[repr(C)]
#[padding_struct]
#[derive(
    Default, Clone, Copy, Pod, Zeroable, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize,
)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 3],
}

#[derive(Default, Clone, Copy)]
#[repr(u8)]
pub enum MaterialType {
    #[default]
    Opaque,
    Transparent,
}

#[derive(Clone, Copy)]
pub struct MaterialState {
    pub material_type: MaterialType,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct MaterialProperties {
    pub base_color: [f32; 4],
    pub metallic_value: f32,
    pub roughness_value: f32,
}

impl MaterialProperties {
    pub fn new(base_color: Vec4, metallic_value: f32, roughness_value: f32) -> Self {
        Self {
            base_color: base_color.to_array(),
            metallic_value,
            roughness_value,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct MaterialTextures {
    pub albedo_texture_index: u32,
    pub metallic_texture_index: u32,
    pub roughness_texture_index: u32,
}

impl MaterialTextures {
    pub fn new(
        albedo_texture_index: u32,
        metallic_texture_index: u32,
        roughness_texture_index: u32,
    ) -> Self {
        Self {
            albedo_texture_index,
            metallic_texture_index,
            roughness_texture_index,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct MaterialData {
    pub material_properties: MaterialProperties,
    pub material_textures: MaterialTextures,
    pub sampler_index: u32,
}

pub struct Material {
    //pub ptr_data: DeviceAddress,
    pub state: MaterialState,
}

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
        math::Mat4 {
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

#[derive(Clone, serde::Serialize, serde::Deserialize, Debug)]
pub struct ModelAssetMetadata {
    uuid: Uuid,
    name: String,
    path_buf: PathBuf,
    // TODO: Temp commenting.
    //textures: Vec<Uuid>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize, Debug)]
pub struct TextureAssetMetadata {
    uuid: Uuid,
    name: String,
    path_buf: PathBuf,
}

pub struct MaterialAssetMetadata {
    uuid: Uuid,
    name: Option<String>,
    path_buf: Option<PathBuf>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub enum AssetMetadata {
    Model(ModelAssetMetadata),
    Texture(TextureAssetMetadata),
}

#[derive(Clone)]
pub struct BaseAssetEntry {
    pub name: String,
    pub extension: String,
    pub path_buf: PathBuf,
}

#[derive(Clone)]
pub struct ModelEntry {
    pub entry: BaseAssetEntry,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub enum TextureFormat {
    RGBA8,
    RGB8,
    Bc1,
    Bc3,
    Bc4,
    Bc5,
    Bc6H,
    Bc7,
}

#[derive(Clone)]
pub struct TextureEntry {
    pub entry: BaseAssetEntry,
    pub format: TextureFormat,
    pub associated_model: Option<ModelEntry>,
}

// TODO: Not sure if it's a good naming.
#[derive(Clone)]
pub enum AssetEntry {
    Model(ModelEntry),
    Texture(TextureEntry),
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

pub fn collect_assets_to_serialize_system(mut importer: ResMut<Importer>) {
    importer.assets_to_serialize.clear();
    importer.meta_files.clear();

    let assets_folder_path = importer.asset_folder_path_buffer.as_path();

    for entry in WalkDir::new(assets_folder_path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            if entry.path().extension().unwrap().to_str().eq(&Some("meta")) {
                let mut metadata_content = String::new();
                std::fs::File::open(entry.path())
                    .unwrap()
                    .read_to_string(&mut metadata_content)
                    .unwrap();
                let meta_file =
                    toml::de::from_str::<AssetMetadata>(metadata_content.as_str()).unwrap();

                importer.meta_files.push(meta_file);
            } else {
                importer
                    .assets_to_serialize
                    .push(entry.path().to_path_buf());
            }
        }
    }
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
            };
            let meta_path = match meta_file {
                AssetMetadata::Model(model_asset) => model_asset.path_buf.as_path(),
                AssetMetadata::Texture(texture_asset_metadata) => {
                    texture_asset_metadata.path_buf.as_path()
                }
            };

            name.eq(meta_name) && path.eq(meta_path)
        })
    });
}

pub fn serialize_unserialized_assets_system(mut importer: ResMut<Importer>) {
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
                        let ktx_texutre = serialize_texture_asset(&mut importer, texture_entry);

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

                        ktx_texutre
                            .write_to_file(serialized_texture_path_buffer.as_path())
                            .unwrap();

                        let texture_asset_metadata = AssetMetadata::Texture(TextureAssetMetadata {
                            uuid,
                            name: texture_entry.entry.name.clone(),
                            path_buf: PathBuf::from(normalized_asset_path),
                            // TODO: Temp commenting.
                            // textures,
                        });
                        let serialized_texture_asset_metadata =
                            toml::ser::to_string_pretty(&texture_asset_metadata).unwrap();

                        let texture_asset_metadata_path = texture_entry.entry.path_buf.clone();

                        std::fs::write(
                            std::format!("{}.meta", texture_asset_metadata_path.display()),
                            serialized_texture_asset_metadata,
                        )
                        .unwrap();
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
            AssetEntry::Texture(model_entry) => {}
        });
}

// TODO: Currently, we serialize and model, and textures, and materials in the same pass, later, need to separate them.
fn serialize_model_asset(
    importer: &mut Importer,
    model_entry: &ModelEntry,
) -> SerializedModeResult {
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
                        vertices.push(Vertex {
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

                    let position_offset = std::mem::offset_of!(Vertex, position);
                    let vertex_stride = std::mem::size_of::<Vertex>();

                    // TODO: Use bytemuck instead.
                    let vertex_data = typed_to_bytes(&vertices);

                    let vertex_data_adapter =
                        VertexDataAdapter::new(vertex_data, vertex_stride, position_offset)
                            .unwrap();

                    optimize_vertex_cache_in_place(&mut indices, vertices.len());
                    let vertices = optimize_vertex_fetch(&mut indices, &vertices);

                    let (meshlets, vertex_indices, triangles) =
                        generate_meshlets(&indices, &vertex_data_adapter);

                    let serialized_mesh = SerializedMesh {
                        vertices,
                        indices: vertex_indices,
                        meshlets,
                        triangles,
                    };

                    mesh_index = serialized_model.meshes.len();

                    let material_index = mesh.material_index();
                    let material = scene.material(material_index).unwrap();

                    let asset_entry = extract_texture(
                        &scene,
                        &model_name,
                        PathBuf::from(model_path),
                        &mut extracted_textures,
                        material.clone(),
                    );

                    let mut texture_format = TextureFormat::Bc1;
                    // TODO: In future, texture format isn't dependent by material type. Type and texture format are independent (Material Type != Texture Format).
                    if is_material_transparent(material) {
                        texture_format = TextureFormat::Bc3;
                    }

                    if let Some(asset_entry) = asset_entry {
                        let texture_entry = TextureEntry {
                            entry: asset_entry.clone(),
                            format: texture_format,
                            associated_model: Some(model_entry.clone()),
                        };

                        associated_texture_entries.push(texture_entry);
                    }

                    entry.insert((mesh, mesh_index));
                    serialized_model.meshes.push(serialized_mesh);
                } else {
                    let already_uploaded_mesh = serialized_meshes.get(&current_mesh_index).unwrap();
                    mesh_name = already_uploaded_mesh.0.name();
                    mesh_index = already_uploaded_mesh.1;
                }

                let serialized_node = SerializedNode {
                    name: mesh_name,
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

    SerializedModeResult {
        serialized_model,
        associated_texture_entries,
    }
}

fn extract_textures_from_model() {}

// TODO: Handle, when texture is not part of model's binary.
fn extract_texture(
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

fn serialize_texture_asset(
    importer: &mut Importer,
    texture_entry: &TextureEntry,
) -> ktx2_rw::Ktx2Texture {
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
        TextureFormat::Bc1 | TextureFormat::Bc3 => ktx2_rw::VkFormat::R8G8B8A8Srgb,
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
        texture_format: texture_entry.format as u32,
        width,
        height,
        mip_levels_count,
    };
    ktx_texture
        .set_metadata(
            stringify!(TextureMetadata),
            bytemuck::bytes_of(&texture_metadata),
        )
        .unwrap();

    for mip_level_index in 0..mip_levels_count {
        texture_data.extend_from_slice(ktx_texture.get_image_data(mip_level_index, 0, 0).unwrap());
    }

    ktx_texture
}

fn is_material_transparent(material: asset_importer::Material) -> bool {
    let alpha_mode = std::str::from_utf8(
        material
            .get_property_raw_ref(c"$mat.gltf.alphaMode", None, 0)
            .unwrap(),
    )
    .unwrap();

    alpha_mode.contains("BLEND")
}

#[inline(always)]
fn get_mesh_indices(node: &Node, num_meshes: usize) -> Vec<usize> {
    let mut mesh_indices = Vec::with_capacity(num_meshes);
    if num_meshes > Default::default() {
        for mesh_index in node.mesh_indices() {
            mesh_indices.push(mesh_index);
        }
    }

    mesh_indices
}

#[inline(always)]
fn generate_meshlets(
    indices: &[u32],
    vertices: &VertexDataAdapter,
) -> (Vec<Meshlet>, Vec<u32>, Vec<u8>) {
    let max_vertices = 64;
    let max_triangles = 64;
    let cone_weight = 0.0;

    let raw_meshlets = build_meshlets(indices, vertices, max_vertices, max_triangles, cone_weight);

    let mut meshlets = Vec::with_capacity(raw_meshlets.len());

    for raw_meshlet in raw_meshlets.meshlets.iter() {
        meshlets.push(Meshlet {
            vertex_offset: raw_meshlet.vertex_offset as _,
            triangle_offset: raw_meshlet.triangle_offset as _,
            vertex_count: raw_meshlet.vertex_count as _,
            triangle_count: raw_meshlet.triangle_count as _,
            ..Default::default()
        });
    }

    (meshlets, raw_meshlets.vertices, raw_meshlets.triangles)
}

/* fn try_upload_texture(
    vulkan_context: &VulkanContextResource,
    renderer_context: &RendererContext,
    textures_pool: &mut TexturesPool,
    buffers_pool: &mut BuffersPool,
    descriptor_set_handle: &mut DescriptorSetHandle,
    scene: &asset_importer::Scene,
    uploaded_textures: &mut HashMap<usize, TextureReference>,
    material: asset_importer::Material,
    texture_reference_to_use: &mut TextureReference,
    model_name: &str,
) {
    if material.texture_count(asset_importer::TextureType::BaseColor) > Default::default() {
        let texture_info = material
            .texture(asset_importer::TextureType::BaseColor, Default::default())
            .unwrap();
        let texture_index = texture_info.path[1..].parse::<usize>().unwrap();

        if let std::collections::hash_map::Entry::Vacant(e) = uploaded_textures.entry(texture_index)
        {
            let texture = scene.texture(texture_index).unwrap();
            let texture_name = texture
                .filename()
                .unwrap_or(std::format!("{model_name}_texture_{texture_index}"));

            let (texture_reference, texture_data) = try_to_load_cached_texture(
                textures_pool,
                model_name,
                texture.clone(),
                &texture_name,
            );
            *texture_reference_to_use = texture_reference;

            vulkan_context.transfer_data_to_image(
                textures_pool.get_image(texture_reference).unwrap(),
                buffers_pool,
                texture_data.as_ptr() as *const _,
                &renderer_context.upload_context,
                Some(texture_data.len()),
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
                texture_name,
                texture_reference.get_index(),
                texture_metadata.width,
                texture_metadata.height,
                1,
            );

            e.insert(texture_reference);
        } else {
            *texture_reference_to_use = *uploaded_textures.get(&texture_index).unwrap();
        }
    }
}

fn try_to_load_cached_texture(
    textures_pool: &mut TexturesPool,
    model_name: &str,
    texture: asset_importer::Texture,
    texture_name: &str,
) -> (TextureReference, Vec<u8>) {
    let mut path = std::path::PathBuf::from("intermediate/textures/");
    path.push(model_name);
    std::fs::create_dir_all(&path).unwrap();

    path.push(String::from_str(texture_name).unwrap());
    let does_exist = std::fs::exists(&path).unwrap();

    let texture_reference: TextureReference;
    let mut texture_data: Vec<u8> = Vec::new();

    if does_exist {
        let texture = Ktx2Texture::from_file(&path).unwrap();
        let texture_metadata_raw: Vec<u8> =
            texture.get_metadata(stringify!(TextureMetadata)).unwrap();
        let texture_metadata = *bytemuck::from_bytes::<TextureMetadata>(&texture_metadata_raw);

        for mip_level_index in 0..texture_metadata.mip_levels_count {
            texture_data.extend_from_slice(texture.get_image_data(mip_level_index, 0, 0).unwrap());
        }

        let extent = Extent3D {
            width: texture_metadata.width,
            height: texture_metadata.height,
            depth: 1,
        };

        let (created_texture_reference, _) = textures_pool.create_texture(
            Some(&mut texture_data),
            true,
            Format::Bc1RgbSrgbBlock,
            extent,
            ImageUsageFlags::Sampled | ImageUsageFlags::TransferDst,
            true,
        );

        texture_reference = created_texture_reference;
    } else {
        let mut data = texture.data_bytes_ref().unwrap();

        let cursor = Cursor::new(&mut data);

        let image = ImageReader::new(cursor)
            .with_guessed_format()
            .unwrap()
            .decode()
            .unwrap();

        let extent = Extent3D {
            width: image.width(),
            height: image.height(),
            depth: 1,
        };
        let rgba_image = image.to_rgba8();
        let mut image_bytes = rgba_image.as_bytes().to_vec();

        let (created_texture_reference, ktx_texture) = textures_pool.create_texture(
            Some(&mut image_bytes),
            false,
            Format::Bc1RgbSrgbBlock,
            extent,
            ImageUsageFlags::Sampled | ImageUsageFlags::TransferDst,
            true,
        );
        texture_reference = created_texture_reference;

        let ktx_texture = ktx_texture.unwrap();
        for mip_level_index in 0..created_texture_reference.texture_metadata.mip_levels_count {
            texture_data
                .extend_from_slice(ktx_texture.get_image_data(mip_level_index, 0, 0).unwrap());
        }

        ktx_texture.write_to_file(path).unwrap();
    }

    (texture_reference, texture_data)
} */
