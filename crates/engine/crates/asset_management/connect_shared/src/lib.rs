use connect_math::*;
use std::path::PathBuf;
use vulkan::vk::Format;

use bevy_ecs::{component::Component, name::Name};
use bytemuck::{Pod, Zeroable};
use padding_struct::padding_struct;
use uuid::Uuid;

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct SerializedMesh {
    // NOTE: Vertices and Inddices baked by meshopt, can be issues with creating colliders, but need to check.
    pub name: String,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub meshlets: Vec<Meshlet>,
    pub triangles: Vec<u8>,
    // FIXME: DO NOT STORE UUID OF MATERIAL, IT'S SHOULD BE IN MODEL ASSET METADATA INSTEAD!
    pub material_uuid: Uuid,
}

#[repr(C)]
#[padding_struct]
#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    Hash,
    PartialEq,
    Eq,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct TextureMetadata {
    pub texture_format: TextureFormat,
    pub width: u32,
    pub height: u32,
    pub mip_levels_count: u32,
}

#[repr(C)]
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct TextureMipMap {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct SerializedTexture {
    pub texture_metadata: TextureMetadata,
    pub texture_mip_maps: Vec<TextureMipMap>,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct SerializedHierarchy {
    pub serialized_nodes: Vec<SerializedNode>,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct SerializedNode {
    pub name: String,
    pub index: usize,
    pub parent_index: Option<usize>,
    pub matrix: [f32; 16],
    pub mesh_index: Option<usize>,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct SerializedModel {
    pub meshes: Vec<SerializedMesh>,
    pub hierarchy: SerializedHierarchy,
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

#[repr(C)]
#[padding_struct]
#[derive(Default, Clone, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct TextureInput {
    pub uuid: Uuid,
    pub offset: usize,
}

#[repr(C)]
#[padding_struct]
#[derive(Default, Clone, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct SerializedMaterial {
    pub size: usize,
    pub data: Vec<u8>,
    pub texture_inputs: Vec<TextureInput>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize, Debug)]
pub struct ModelAssetMetadata {
    pub uuid: Uuid,
    pub name: String,
    pub path_buf: PathBuf,
    //materials: Vec<Uuid>,
    // TODO: Temp comment1ing.
    //textures: Vec<Uuid>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize, Debug)]
pub struct TextureAssetMetadata {
    pub uuid: Uuid,
    pub name: String,
    pub path_buf: PathBuf,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct MaterialAssetMetadata {
    pub uuid: Uuid,
    pub name: String,
    pub path_buf: PathBuf,
    pub textures: Vec<Uuid>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub enum AssetMetadata {
    Model(ModelAssetMetadata),
    Texture(TextureAssetMetadata),
    Material(MaterialAssetMetadata),
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
#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    Hash,
    PartialEq,
    Eq,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub enum TextureFormat {
    #[default]
    RGBA8Srgb,
    RGB8Srgb,
    RGBA8Unorm,
    RGBA16Sfloat,
    D32Sfloat,
    Bc1,
    Bc3,
    Bc4,
    Bc5,
    Bc6H,
    Bc7,
}

impl TryFrom<Format> for TextureFormat {
    type Error = Format;

    fn try_from(v: Format) -> Result<Self, Self::Error> {
        match v {
            Format::R8G8B8A8_SRGB => Ok(TextureFormat::RGBA8Srgb),
            Format::R8G8B8_SRGB => Ok(TextureFormat::RGB8Srgb),
            Format::R8G8B8A8_UNORM => Ok(TextureFormat::RGBA8Unorm),
            Format::R16G16B16A16_SFLOAT => Ok(TextureFormat::RGBA16Sfloat),
            Format::D32_SFLOAT => Ok(TextureFormat::D32Sfloat),
            Format::BC1_RGB_SRGB_BLOCK => Ok(TextureFormat::Bc1),
            Format::BC3_SRGB_BLOCK => Ok(TextureFormat::Bc3),
            Format::BC4_UNORM_BLOCK => Ok(TextureFormat::Bc4),
            Format::BC5_UNORM_BLOCK => Ok(TextureFormat::Bc5),
            Format::BC6H_SFLOAT_BLOCK => Ok(TextureFormat::Bc6H),
            Format::BC7_SRGB_BLOCK => Ok(TextureFormat::Bc7),
            _ => Err(v),
        }
    }
}

impl TryInto<Format> for TextureFormat {
    type Error = Self;

    fn try_into(self) -> Result<Format, Self::Error> {
        match self {
            TextureFormat::RGBA8Srgb => Ok(Format::R8G8B8A8_SRGB),
            TextureFormat::RGB8Srgb => Ok(Format::R8G8B8_SRGB),
            TextureFormat::RGBA8Unorm => Ok(Format::R8G8B8A8_UNORM),
            TextureFormat::RGBA16Sfloat => Ok(Format::R16G16B16A16_SFLOAT),
            TextureFormat::D32Sfloat => Ok(Format::D32_SFLOAT),
            TextureFormat::Bc1 => Ok(Format::BC1_RGB_SRGB_BLOCK),
            TextureFormat::Bc3 => Ok(Format::BC3_SRGB_BLOCK),
            TextureFormat::Bc4 => Ok(Format::BC4_UNORM_BLOCK),
            TextureFormat::Bc5 => Ok(Format::BC5_UNORM_BLOCK),
            TextureFormat::Bc6H => Ok(Format::BC6H_SFLOAT_BLOCK),
            TextureFormat::Bc7 => Ok(Format::BC7_SRGB_BLOCK),
        }
    }
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

slotmap::new_key_type! {
    pub struct BufferKey;
    pub struct TextureKey;
    pub struct SamplerKey;
    pub struct MeshBufferKey;
    pub struct MeshDataKey;
    pub struct MaterialKey;
    pub struct AudioKey;
}

pub struct AssetsExtensions;

impl AssetsExtensions {
    pub const META_FILE_EXTENSION: &'static str = "meta";
}

pub struct ArtifactsFoldersNames;

impl ArtifactsFoldersNames {
    pub const MODELS_FOLDER_NAME: &'static str = "models";
    pub const TEXTURES_FOLDER_NAME: &'static str = "textures";
    pub const MATERIALS_FOLDER_NAME: &'static str = "materials";
}

// TODO: MOVE TO SOME PLACE

#[derive(Clone, Copy, Component, Debug)]
#[require(GlobalTransform, Name)]
pub struct LocalTransform {
    pub local_position: Vec3,
    pub local_rotation: Quat,
    pub local_scale: Vec3,
}

impl LocalTransform {
    pub const IDENTITY: LocalTransform = LocalTransform {
        local_position: Vec3::ZERO,
        local_rotation: Quat::IDENTITY,
        local_scale: Vec3::ONE,
    };

    pub fn new(position: Vec3, rotation: Quat, scale: Vec3) -> Self {
        Self {
            local_position: position,
            local_rotation: rotation,
            local_scale: scale,
        }
    }

    pub fn get_local_position(&self) -> Vec3 {
        self.local_position
    }

    pub fn set_local_position(&mut self, pos: Vec3) {
        self.local_position = pos;
    }

    pub fn get_local_rotation(&self) -> Quat {
        self.local_rotation
    }

    pub fn set_local_rotation(&mut self, rot: Quat) {
        self.local_rotation = rot;
    }

    pub fn get_local_euler_angles(&self) -> Vec3 {
        let (y, x, z) = self.local_rotation.to_euler(EulerRot::YXZ);
        Vec3::new(x.to_degrees(), y.to_degrees(), z.to_degrees())
    }

    pub fn set_local_euler_angles(&mut self, euler_degrees: Vec3) {
        let x_rad = euler_degrees.x.to_radians();
        let y_rad = euler_degrees.y.to_radians();
        let z_rad = euler_degrees.z.to_radians();

        self.local_rotation = Quat::from_euler(EulerRot::YXZ, y_rad, x_rad, z_rad);
    }

    pub fn forward(&self) -> Vec3 {
        let mut forward = self.local_rotation * Vec3::NEG_Z;
        forward.y = Default::default();

        forward
    }

    pub fn right(&self) -> Vec3 {
        let mut right = self.local_rotation * Vec3::X;
        right.y = Default::default();

        right
    }

    pub fn up(&self) -> Vec3 {
        self.local_rotation * Vec3::Y
    }

    pub fn translate_local(&mut self, translation: Vec3) {
        self.local_position += self.local_rotation * translation;
    }

    pub fn look_at(&mut self, target: Vec3, world_up: Vec3) {
        let forward = (target - self.local_position).normalize_or_zero();
        if forward == Vec3::ZERO {
            return;
        }

        let rotation_matrix = Mat4::look_at_rh(Vec3::ZERO, forward, world_up).inverse();
        self.local_rotation = Quat::from_mat4(&rotation_matrix);
    }

    pub fn local_to_world_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            self.local_scale,
            self.local_rotation,
            self.local_position,
        )
    }
}

impl Default for LocalTransform {
    fn default() -> Self {
        LocalTransform::IDENTITY
    }
}

#[derive(Component, Clone, Copy)]
pub struct GlobalTransform(pub Mat4);

impl Default for GlobalTransform {
    fn default() -> Self {
        Self(Mat4::from_scale_rotation_translation(
            Vec3::ONE,
            Quat::IDENTITY,
            Vec3::ZERO,
        ))
    }
}
