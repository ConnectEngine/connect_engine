pub mod audio;
pub mod buffers_pool;
pub mod materials_pool;
pub mod mesh_buffers_pool;
pub mod model_loader;
pub mod physics;
pub mod samplers_pool;
pub mod textures_pool;

use bevy_ecs::resource::Resource;
use bytemuck::{NoUninit, Pod, Zeroable};
use math::{Vec3, Vec4};
use padding_struct::padding_struct;
use slotmap::new_key_type;
use vulkanite::vk::{rs::*, *};

use crate::engine::resources::{
    buffers_pool::BufferReference, render_resources::model_loader::ModelLoader,
    samplers_pool::SamplerReference, textures_pool::TextureReference,
};

#[repr(C)]
#[padding_struct]
#[derive(Default, Clone, Copy, Pod, Zeroable)]
pub struct Meshlet {
    pub vertex_offset: u32,
    pub triangle_offset: u32,
    pub vertex_count: u32,
    pub triangle_count: u32,
}

#[repr(C)]
#[padding_struct]
#[derive(Default, Clone, Copy, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 3],
}

#[repr(C)]
#[padding_struct]
#[derive(Default, Clone, Copy, Pod, Zeroable)]
pub struct MeshObject {
    pub device_address_vertex_buffer: DeviceAddress,
    pub device_address_vertex_indices_buffer: DeviceAddress,
    pub device_address_meshlets_buffer: DeviceAddress,
    pub device_address_local_indices_buffer: DeviceAddress,
}

#[repr(C)]
#[padding_struct]
#[derive(Default, Clone, Copy, Pod, Zeroable)]
pub struct InstanceObject {
    pub model_matrix: [f32; 16],
    pub device_address_mesh_object: DeviceAddress,
    pub device_address_material_data: DeviceAddress,
    pub meshlet_count: u32,
    pub material_type: u8,
}

#[repr(C)]
#[padding_struct]
#[derive(Clone, Copy, Default, Pod, Zeroable)]
pub struct GraphicsPushConstant {
    pub device_address_scene_data: DeviceAddress,
    pub device_address_instance_object: DeviceAddress,
    pub draw_image_index: u32,
    pub current_material_type: u32,
}

#[derive(Default, Clone, Copy)]
pub struct ShaderObject {
    pub shader: Option<ShaderEXT>,
    pub stage: ShaderStageFlags,
}

impl ShaderObject {
    pub fn new(shader: Option<ShaderEXT>, stage: ShaderStageFlags) -> Self {
        Self { shader, stage }
    }
}

#[repr(C)]
#[padding_struct]
#[derive(Default, Clone, Copy, Pod, Zeroable)]
pub struct LightProperties {
    pub ambient_color: Vec4,
    pub ambient_strength: f32,
    pub specular_strength: f32,
}

#[repr(C)]
#[padding_struct]
#[derive(Default, Clone, Copy, Pod, Zeroable)]
pub struct DirectionalLight {
    pub light_color: Vec3,
    pub light_position: Vec3,
}

#[repr(C)]
#[padding_struct]
#[derive(Default, Clone, Copy, Pod, Zeroable)]
pub struct SceneData {
    pub camera_view_matrix: [f32; 16],
    pub camera_position: Vec3,
    pub light_properties: LightProperties,
    pub directional_light: DirectionalLight,
}

pub struct SwappableBuffer<T: NoUninit + Pod + Sized> {
    current_buffer_index: usize,
    buffers: Vec<BufferReference>,
    objects: Vec<T>,
    objects_to_write: Vec<u8>,
}

impl<'a, T: bytemuck::Pod> SwappableBuffer<T> {
    pub fn new(buffers: Vec<BufferReference>) -> Self {
        Self {
            current_buffer_index: Default::default(),
            buffers,
            objects: Default::default(),
            objects_to_write: Default::default(),
        }
    }

    pub fn next_buffer(&mut self) {
        self.current_buffer_index += 1;
        if self.current_buffer_index >= self.buffers.len() {
            self.current_buffer_index = Default::default();
        }
        self.objects.clear();
    }

    pub fn get_current_buffer(&self) -> BufferReference {
        unsafe { *self.buffers.get_unchecked(self.current_buffer_index) }
    }

    pub fn get_objects_to_write_as_slice(&'a self) -> &'a [u8] {
        &self.objects_to_write
    }

    pub fn prepare_objects_for_writing(&mut self) {
        let object_to_write = bytemuck::cast_slice(&self.objects);
        self.objects_to_write.extend_from_slice(object_to_write);
    }

    pub fn add_instance_object(&mut self, object_to_write: T) {
        self.objects.push(object_to_write);
    }

    pub fn clear(&mut self) {
        self.objects.clear();
        self.objects_to_write.clear();
    }
}

pub struct ResourcesPool {
    pub instances_buffer: Option<SwappableBuffer<InstanceObject>>,
    pub scene_data_buffer: Option<SwappableBuffer<SceneData>>,
}

impl ResourcesPool {
    pub fn new() -> Self {
        Self {
            instances_buffer: Default::default(),
            scene_data_buffer: Default::default(),
        }
    }
}

#[derive(Resource)]
pub struct RendererResources {
    pub default_texture_reference: TextureReference,
    pub fallback_texture_reference: TextureReference,
    pub default_sampler_reference: SamplerReference,
    // TODO: Move to mesh buffers pool
    pub mesh_objects_buffer_reference: BufferReference,
    pub materials_data_buffer_reference: BufferReference,
    pub gradient_compute_shader_object: ShaderObject,
    pub task_shader_object: ShaderObject,
    pub mesh_shader_object: ShaderObject,
    pub fragment_shader_object: ShaderObject,
    pub model_loader: ModelLoader,
    pub resources_pool: ResourcesPool,
    pub is_printed_scene_hierarchy: bool,
}
