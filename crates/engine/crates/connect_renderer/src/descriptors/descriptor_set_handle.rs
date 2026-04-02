use std::{mem::ManuallyDrop, sync::Arc};

use ahash::HashMap;
use bevy_ecs::resource::Resource;
use vulkan::{Device, vk::*};

use crate::*;

#[derive(Default)]
pub struct DescriptorSetLayoutHandle {
    pub descriptor_set_layout: DescriptorSetLayout,
    pub descriptor_set_layout_size: u64,
}

#[derive(Default, Clone, Copy)]
pub struct DescriptorsSizes {
    pub sampled_image_descriptor_size: usize,
    pub sampler_descriptor_size: usize,
    pub storage_image_descriptor_size: usize,
}

#[derive(Clone, Copy)]
pub struct BindingInfo {
    pub binding_offset: DeviceSize,
}

// TODO: Change name to more convinient.
#[derive(Resource)]
pub struct DescriptorSetHandle {
    device: Arc<Device>,
    pub descriptor_buffer_reference: BufferReference,
    pub descriptor_set_layout_handle: DescriptorSetLayoutHandle,
    pub push_contant_ranges: Vec<PushConstantRange>,
    pub bindings_infos: HashMap<DescriptorType, BindingInfo>,
    pub pipeline_layout: PipelineLayout,
    pub descriptors_sizes: DescriptorsSizes,
}

impl DescriptorSetHandle {
    pub fn new(device: Arc<Device>) -> Self {
        Self {
            device,
            descriptor_buffer_reference: Default::default(),
            descriptor_set_layout_handle: Default::default(),
            push_contant_ranges: Default::default(),
            bindings_infos: Default::default(),
            pipeline_layout: Default::default(),
            descriptors_sizes: Default::default(),
        }
    }

    pub fn update_binding(
        &mut self,
        buffers_pool: &BuffersPoolResource,
        descriptor_kind: DescriptorKind,
    ) {
        let descriptor_type = descriptor_kind.get_descriptor_type();

        let descriptors_sizes = self.descriptors_sizes;
        let descriptor_size = match descriptor_type {
            DescriptorType::SAMPLED_IMAGE => descriptors_sizes.sampled_image_descriptor_size,
            DescriptorType::STORAGE_IMAGE => descriptors_sizes.storage_image_descriptor_size,
            DescriptorType::SAMPLER => descriptors_sizes.sampler_descriptor_size,
            unsupported_descriptor_type => panic!(
                "Unsupported Descriptor Type found: {:?}",
                unsupported_descriptor_type
            ),
        };

        let binding_info = self.bindings_infos.get_mut(&descriptor_type).unwrap();

        // TODO: Temp before migration to fully slot architecture.
        let descriptor_slot_index = match descriptor_kind {
            DescriptorKind::StorageImage(descriptor_storage_image) => {
                descriptor_storage_image.index
            }
            DescriptorKind::SampledImage(descriptor_sampled_image) => {
                descriptor_sampled_image.index
            }
            DescriptorKind::Sampler(descriptor_sampler) => descriptor_sampler.index,
        };

        let base_binding_offset = binding_info.binding_offset;
        let binding_offset =
            base_binding_offset + (descriptor_slot_index as u64 * descriptor_size as u64);

        let mapped_allocation = buffers_pool.map_allocation(self.descriptor_buffer_reference);

        let target_descriptor_buffer_address =
            unsafe { mapped_allocation.get_ptr().add(binding_offset as usize) };

        let target_descriptor_buffer = unsafe {
            std::slice::from_raw_parts_mut(target_descriptor_buffer_address, descriptor_size)
        };

        let mut descriptor_data = DescriptorDataEXT::default();
        let mut descriptor_get_info = DescriptorGetInfoEXT::default();

        match descriptor_kind {
            DescriptorKind::StorageImage(descriptor_storage_image) => {
                let storage_image_descriptor_info = DescriptorImageInfo {
                    image_view: descriptor_storage_image.image_view,
                    image_layout: ImageLayout::GENERAL,
                    ..Default::default()
                };

                let p_storage_image_descriptor_info =
                    &storage_image_descriptor_info as *const _ as _;
                descriptor_data.storage_image = p_storage_image_descriptor_info;

                descriptor_get_info.type_ = DescriptorType::STORAGE_IMAGE;
                descriptor_get_info.data = descriptor_data;

                unsafe {
                    self.device
                        .get_descriptor_ext(&descriptor_get_info, target_descriptor_buffer);
                }
            }
            DescriptorKind::SampledImage(descriptor_sampled_image) => {
                let sampled_image_descriptor_info = DescriptorImageInfo {
                    image_view: descriptor_sampled_image.image_view,
                    image_layout: ImageLayout::GENERAL,
                    ..Default::default()
                };

                let p_sampled_image_descriptor_info =
                    &sampled_image_descriptor_info as *const _ as _;
                descriptor_data.sampled_image = p_sampled_image_descriptor_info;

                descriptor_get_info.type_ = DescriptorType::SAMPLED_IMAGE;
                descriptor_get_info.data = descriptor_data;

                unsafe {
                    self.device
                        .get_descriptor_ext(&descriptor_get_info, target_descriptor_buffer);
                }
            }
            DescriptorKind::Sampler(descriptor_sampler) => {
                let p_sampler = &descriptor_sampler.sampler as *const _ as _;
                descriptor_data.sampler = p_sampler;

                descriptor_get_info.type_ = DescriptorType::SAMPLER;
                descriptor_get_info.data = descriptor_data;

                unsafe {
                    self.device
                        .get_descriptor_ext(&descriptor_get_info, target_descriptor_buffer);
                }
            }
        };
    }

    pub fn get_pipeline_layout(&self) -> PipelineLayout {
        self.pipeline_layout
    }

    pub fn get_descriptor_set_layout(&self) -> DescriptorSetLayout {
        self.descriptor_set_layout_handle.descriptor_set_layout
    }

    pub fn get_buffer_info(&self) -> BufferInfo {
        self.descriptor_buffer_reference.get_buffer_info()
    }

    pub fn destroy(&self) {
        let device = &self.device;

        unsafe {
            device.destroy_pipeline_layout(self.pipeline_layout, None);

            device.destroy_descriptor_set_layout(
                self.descriptor_set_layout_handle.descriptor_set_layout,
                None,
            );
        }
    }
}
