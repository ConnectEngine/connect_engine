use bevy_ecs::{component::Component, resource::Resource};
use shared::{LocalTransform, MeshBufferKey, Vertex};
use slotmap::{Key, SlotMap};
use vulkanite::vk::DeviceAddress;

use crate::resources::{buffers_pool::BufferReference, materials_pool::MaterialReference};

// TODO: MOVE TO SOME PLACE

#[derive(Component)]
pub struct MeshData {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

#[derive(Component, Clone, Copy)]
#[require(LocalTransform)]
pub struct Mesh {
    pub(crate) mesh_buffer_reference: MeshBufferReference,
    pub(crate) material_reference: MaterialReference,
}

//////////////////////////////////////////////////

pub struct MeshBuffer {
    pub mesh_object_device_address: DeviceAddress,
    pub vertex_buffer_reference: BufferReference,
    pub vertex_indices_buffer_reference: BufferReference,
    pub meshlets_buffer_reference: BufferReference,
    pub local_indices_buffer_reference: BufferReference,
    pub meshlets_count: usize,
    pub mesh_data: MeshData,
}

#[derive(Clone, Copy)]
pub struct MeshBufferReference {
    key: MeshBufferKey,
}

impl MeshBufferReference {
    pub fn get_index(&self) -> u32 {
        self.key.data().get_key() - 1
    }
}

#[derive(Resource)]
pub struct MeshBuffersPool {
    slots: SlotMap<MeshBufferKey, MeshBuffer>,
}

impl MeshBuffersPool {
    pub fn new(pre_allocated_count: usize) -> Self {
        Self {
            slots: SlotMap::with_capacity_and_key(pre_allocated_count),
        }
    }

    pub fn insert_mesh_buffer(&mut self, mesh_buffer: MeshBuffer) -> MeshBufferReference {
        let mesh_buffer_key = self.slots.insert(mesh_buffer);

        MeshBufferReference {
            key: mesh_buffer_key,
        }
    }

    pub fn get_mesh_buffer(
        &self,
        mesh_buffer_reference: MeshBufferReference,
    ) -> Option<&MeshBuffer> {
        self.slots.get(mesh_buffer_reference.key)
    }

    pub fn get_mesh_buffer_mut(
        &mut self,
        mesh_buffer_reference: MeshBufferReference,
    ) -> Option<&mut MeshBuffer> {
        self.slots.get_mut(mesh_buffer_reference.key)
    }
}
