use ahash::AHashMap;
use bevy_ecs::resource::Resource;
use connect_math::*;
use connect_shared::MaterialKey;
use slotmap::SlotMap;
use vulkanite::vk::DeviceAddress;

// TODO: MOVE TO SOME PLACE
use bytemuck::{Pod, Zeroable};

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
    pub ptr_data: DeviceAddress,
    pub state: MaterialState,
}

///////////////////////////////////////////////////////////////////////

#[derive(Clone, Copy)]
pub struct OffsetElement {
    pub size: usize,
    pub offset: usize,
}

pub struct VariableOffsets {
    offsets: AHashMap<u32, OffsetElement>,
    current_offset: usize,
}

impl VariableOffsets {
    pub fn new(capacity: usize) -> Self {
        VariableOffsets {
            offsets: AHashMap::with_capacity(capacity),
            current_offset: Default::default(),
        }
    }

    pub fn push(&mut self, index: u32, size: usize) -> OffsetElement {
        let offset_element = OffsetElement {
            size,
            offset: self.current_offset,
        };
        self.current_offset = offset_element.size + offset_element.offset;
        self.offsets.insert(index, offset_element);

        offset_element
    }
}

#[derive(Copy, Clone, Default, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct MaterialReference {
    key: MaterialKey,
}

impl MaterialReference {
    pub fn new(key: MaterialKey) -> Self {
        Self { key }
    }

    pub fn get_index(&self) -> u32 {
        self.key.0.get_key() - 1
    }
}

pub struct MaterialInstance {
    material_state: MaterialState,
    offset_element: Option<OffsetElement>,
}

impl MaterialInstance {
    pub fn new(material_state: MaterialState, offset_element: Option<OffsetElement>) -> Self {
        Self {
            material_state,
            offset_element,
        }
    }

    pub fn get_material_state(&self) -> MaterialState {
        self.material_state
    }

    pub fn get_size(&self) -> usize {
        self.offset_element.unwrap().size
    }

    pub fn get_offset(&self) -> usize {
        self.offset_element.unwrap().offset
    }

    pub fn set_offset_element(&mut self, offset_element: Option<OffsetElement>) {
        self.offset_element = offset_element;
    }
}

pub struct MaterialInfo {
    pub material_type: MaterialType,
    pub device_adddress_material_data: DeviceAddress,
    pub size: usize,
}

#[derive(Resource)]
pub struct MaterialsPool {
    slots: SlotMap<MaterialKey, MaterialInstance>,
    materials_to_write: AHashMap<MaterialReference, Vec<u8>>,
    base_device_address_material_data: DeviceAddress,
    variable_offsets: VariableOffsets,
}

impl MaterialsPool {
    pub fn new(
        base_device_address_material_data: DeviceAddress,
        pre_allocated_count: usize,
    ) -> Self {
        Self {
            slots: SlotMap::with_capacity_and_key(pre_allocated_count),
            materials_to_write: AHashMap::with_capacity(1024),
            variable_offsets: VariableOffsets::new(pre_allocated_count),
            base_device_address_material_data,
        }
    }

    pub fn write_material(
        &mut self,
        data: &[u8],
        material_state: MaterialState,
    ) -> MaterialReference {
        let material_instance = MaterialInstance::new(material_state, None);
        let material_key = self.slots.insert(material_instance);
        let material_reference = MaterialReference::new(material_key);

        let offset_element = self
            .variable_offsets
            .push(material_reference.get_index(), data.len());

        unsafe {
            self.slots
                .get_mut(material_reference.key)
                .unwrap_unchecked()
                .set_offset_element(Some(offset_element));
        }
        self.materials_to_write
            .insert(material_reference, data.to_vec());

        material_reference
    }

    pub fn reset_materails_to_write(&mut self) {
        self.materials_to_write.clear();
    }

    pub fn get_materials_data_to_write(&self) -> &AHashMap<MaterialReference, Vec<u8>> {
        &self.materials_to_write
    }

    pub fn get_material_instance(
        &self,
        material_reference: MaterialReference,
    ) -> Option<&MaterialInstance> {
        self.slots.get(material_reference.key)
    }

    pub fn get_material_info(&self, material_reference: MaterialReference) -> MaterialInfo {
        let material_instance = self.slots.get(material_reference.key).unwrap();

        MaterialInfo {
            material_type: material_instance.material_state.material_type,
            device_adddress_material_data: self.base_device_address_material_data
                + material_instance.get_offset() as u64,
            size: material_instance.get_size(),
        }
    }
}
