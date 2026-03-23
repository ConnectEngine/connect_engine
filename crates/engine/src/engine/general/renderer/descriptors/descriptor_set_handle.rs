use std::mem::ManuallyDrop;

use ahash::HashMap;
use bevy_ecs::resource::Resource;

use vulkanite::{
    Handle,
    vk::{rs::*, *},
};

use crate::engine::general::renderer::DescriptorKind;

use renderer::*;

#[derive(Default)]
pub struct DescriptorSetLayoutHandle {
    pub descriptor_set_layout: Option<DescriptorSetLayout>,
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

#[derive(Resource)]
pub struct DescriptorSetHandle {
    device: Device,
    pub descriptor_buffer_reference: BufferReference,
    pub descriptor_set_layout_handle: DescriptorSetLayoutHandle,
    pub push_contant_ranges: Vec<PushConstantRange>,
    pub bindings_infos: HashMap<u32, BindingInfo>,
    pub pipeline_layout: Option<PipelineLayout>,
    pub descriptors_sizes: DescriptorsSizes,
}

impl DescriptorSetHandle {
    pub fn new(device: Device) -> Self {
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

    pub fn update_binding(&mut self, buffers_pool: &BuffersPool, descriptor_kind: DescriptorKind) {
        let descriptor_type = descriptor_kind.get_descriptor_type();

        let descriptors_sizes = self.descriptors_sizes;
        let descriptor_size = match descriptor_type {
            DescriptorType::SampledImage => descriptors_sizes.sampled_image_descriptor_size,
            DescriptorType::StorageImage => descriptors_sizes.storage_image_descriptor_size,
            DescriptorType::Sampler => descriptors_sizes.sampler_descriptor_size,
            unsupported_descriptor_type => panic!(
                "Unsupported Descriptor Type found: {:?}",
                unsupported_descriptor_type
            ),
        };

        let descriptor_type_raw = descriptor_type as u32;
        let binding_info = self.bindings_infos.get_mut(&descriptor_type_raw).unwrap();

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

        let mut descriptor_data = DescriptorDataEXT::default();
        let mut descriptor_get_info = DescriptorGetInfoEXT::default();

        match descriptor_kind {
            DescriptorKind::StorageImage(descriptor_storage_image) => {
                let storage_image_descriptor_info = DescriptorImageInfo {
                    image_view: Some(descriptor_storage_image.image_view.borrow()),
                    image_layout: ImageLayout::General,
                    ..Default::default()
                };

                let mut p_storage_image_descriptor_info =
                    ManuallyDrop::new(&storage_image_descriptor_info as *const _ as _);
                descriptor_data.p_storage_image = p_storage_image_descriptor_info;

                descriptor_get_info.ty = DescriptorType::StorageImage;
                descriptor_get_info.data = descriptor_data;

                self.device.get_descriptor_ext(
                    &descriptor_get_info,
                    descriptor_size,
                    target_descriptor_buffer_address as _,
                );

                unsafe {
                    ManuallyDrop::drop(&mut p_storage_image_descriptor_info);
                }
            }
            DescriptorKind::SampledImage(descriptor_sampled_image) => {
                let sampled_image_descriptor_info = DescriptorImageInfo {
                    image_view: Some(descriptor_sampled_image.image_view.borrow()),
                    image_layout: ImageLayout::General,
                    ..Default::default()
                };

                let mut p_sampled_image_descriptor_info =
                    ManuallyDrop::new(&sampled_image_descriptor_info as *const _ as _);
                descriptor_data.p_sampled_image = p_sampled_image_descriptor_info;

                descriptor_get_info.ty = DescriptorType::SampledImage;
                descriptor_get_info.data = descriptor_data;

                self.device.get_descriptor_ext(
                    &descriptor_get_info,
                    descriptor_size,
                    target_descriptor_buffer_address as _,
                );

                unsafe {
                    ManuallyDrop::drop(&mut p_sampled_image_descriptor_info);
                }
            }
            DescriptorKind::Sampler(descriptor_sampler) => {
                let mut p_sampler = ManuallyDrop::new(&descriptor_sampler.sampler as *const _ as _);
                descriptor_data.p_sampler = p_sampler;

                descriptor_get_info.ty = DescriptorType::Sampler;
                descriptor_get_info.data = descriptor_data;

                self.device.get_descriptor_ext(
                    &descriptor_get_info,
                    descriptor_size,
                    target_descriptor_buffer_address as _,
                );

                unsafe {
                    ManuallyDrop::drop(&mut p_sampler);
                }
            }
        };
    }

    pub fn get_pipeline_layout(&self) -> PipelineLayout {
        unsafe { self.pipeline_layout.unwrap_unchecked() }
    }

    pub fn get_descriptor_set_layout(&self) -> DescriptorSetLayout {
        unsafe {
            self.descriptor_set_layout_handle
                .descriptor_set_layout
                .unwrap_unchecked()
        }
    }

    pub fn get_buffer_info(&self) -> BufferInfo {
        self.descriptor_buffer_reference.get_buffer_info()
    }

    pub fn destroy(&self) {
        let device = self.device;

        unsafe {
            device.destroy_pipeline_layout(self.pipeline_layout);

            device.destroy_descriptor_set_layout(
                self.descriptor_set_layout_handle.descriptor_set_layout,
            );
        }
    }
}
