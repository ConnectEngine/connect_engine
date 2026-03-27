use std::collections::HashMap;

use vma::*;
use vulkanite::vk::{rs::*, *};

use crate::*;

pub enum DescriptorKind {
    StorageImage(DescriptorStorageImage),
    SampledImage(DescriptorSampledImage),
    Sampler(DescriptorSampler),
}

impl DescriptorKind {
    pub fn get_descriptor_type(&self) -> DescriptorType {
        match self {
            DescriptorKind::StorageImage(_) => DescriptorType::StorageImage,
            DescriptorKind::SampledImage(_) => DescriptorType::SampledImage,
            DescriptorKind::Sampler(_) => DescriptorType::Sampler,
        }
    }
}

struct DescriptorSetLayoutBindingInfo<'a> {
    pub binding: DescriptorSetLayoutBinding<'a>,
    pub flags: DescriptorBindingFlags,
}

#[derive(Default)]
pub struct DescriptorSetBuilder<'a> {
    bindings_infos: Vec<DescriptorSetLayoutBindingInfo<'a>>,
}

impl<'a> DescriptorSetBuilder<'a> {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn add_binding(
        mut self,
        descriptor_type: DescriptorType,
        descriptor_count: u32,
        binding_flags: DescriptorBindingFlags,
    ) -> Self {
        let next_binding_index = self.bindings_infos.len();
        let binding = DescriptorSetLayoutBinding::default()
            .binding(next_binding_index as _)
            .descriptor_type(descriptor_type)
            .descriptor_count(descriptor_count);

        let binding_info = DescriptorSetLayoutBindingInfo {
            binding,
            flags: binding_flags,
        };

        self.bindings_infos.push(binding_info);

        self
    }

    pub fn build(
        mut self,
        device: Device,
        _allocator: Allocator,
        buffers_pool: &mut BuffersPool,
        descriptor_buffer_properties: &PhysicalDeviceDescriptorBufferPropertiesEXT,
        push_constant_ranges: &[PushConstantRange],
        shader_stages: ShaderStageFlags,
    ) -> DescriptorSetHandle {
        let descriptor_set_layout_handle = self.create_descriptor_set_layout(
            device,
            shader_stages,
            DescriptorSetLayoutCreateFlags::DescriptorBufferEXT,
        );

        let descriptor_set_layouts = [descriptor_set_layout_handle.descriptor_set_layout.unwrap()];

        let mut bindings_infos: HashMap<u32, BindingInfo, ahash::RandomState> =
            HashMap::with_hasher(ahash::RandomState::new());

        self.bindings_infos.iter().enumerate().for_each(
            |(binding_index, descriptor_set_layout_binding_info)| {
                let binding_offset = device.get_descriptor_set_layout_binding_offset_ext(
                    *descriptor_set_layouts.first().unwrap(),
                    binding_index as _,
                );

                let binding_info = BindingInfo { binding_offset };
                bindings_infos.insert(
                    descriptor_set_layout_binding_info.binding.descriptor_type as _,
                    binding_info,
                );
            },
        );

        let descriptor_buffer_size = Self::get_descriptor_buffer_aligned_size(
            descriptor_set_layout_handle.descriptor_set_layout_size,
            descriptor_buffer_properties.descriptor_buffer_offset_alignment,
        );

        let descriptor_buffer_reference = buffers_pool.create_buffer(
            descriptor_buffer_size as _,
            BufferUsageFlags::ShaderDeviceAddress | BufferUsageFlags::ResourceDescriptorBufferEXT,
            BufferVisibility::HostVisible,
            None,
            Some("Descriptor Set".to_string()),
        );

        let pipeline_layout_info = PipelineLayoutCreateInfo::default()
            .set_layouts(descriptor_set_layouts.as_slice())
            .push_constant_ranges(push_constant_ranges);
        let pipeline_layout = device
            .create_pipeline_layout(&pipeline_layout_info)
            .unwrap();

        let sampled_image_descriptor_size =
            descriptor_buffer_properties.sampled_image_descriptor_size;
        let storage_image_descriptor_size =
            descriptor_buffer_properties.storage_image_descriptor_size;
        let sampler_descriptor_size = descriptor_buffer_properties.sampler_descriptor_size;

        let descriptor_sizes = DescriptorsSizes {
            sampled_image_descriptor_size,
            sampler_descriptor_size,
            storage_image_descriptor_size,
        };

        let mut descriptor_set_handle = DescriptorSetHandle::new(device);
        descriptor_set_handle.descriptor_buffer_reference = descriptor_buffer_reference;
        descriptor_set_handle.descriptor_set_layout_handle = descriptor_set_layout_handle;
        descriptor_set_handle.push_contant_ranges = push_constant_ranges.to_vec();
        descriptor_set_handle.pipeline_layout = Some(pipeline_layout);
        descriptor_set_handle.bindings_infos = bindings_infos;
        descriptor_set_handle.descriptors_sizes = descriptor_sizes;

        descriptor_set_handle
    }

    fn create_descriptor_set_layout(
        &mut self,
        device: Device,
        shader_stages: ShaderStageFlags,
        descriptor_set_layout_flags: DescriptorSetLayoutCreateFlags,
    ) -> DescriptorSetLayoutHandle {
        let mut bindings_flags: Vec<DescriptorBindingFlags> =
            Vec::with_capacity(self.bindings_infos.len());

        let bindings: Vec<_> = self
            .bindings_infos
            .iter_mut()
            .map(|binding_info| {
                let binding = &binding_info.binding;

                bindings_flags.push(binding_info.flags);
                DescriptorSetLayoutBinding {
                    binding: binding.binding,
                    descriptor_type: binding.descriptor_type,
                    descriptor_count: binding_info.binding.descriptor_count,
                    stage_flags: binding.stage_flags | shader_stages,
                    ..Default::default()
                }
            })
            .collect();

        let descriptor_set_layout_binding_flags_create_info =
            &mut DescriptorSetLayoutBindingFlagsCreateInfo::default()
                .binding_count(bindings_flags.len() as _)
                .binding_flags(&bindings_flags);

        let descriptor_set_layout_info = DescriptorSetLayoutCreateInfo::default()
            .flags(descriptor_set_layout_flags)
            .bindings(&bindings)
            .push_next(descriptor_set_layout_binding_flags_create_info);

        let descriptor_set_layout = device
            .create_descriptor_set_layout(&descriptor_set_layout_info)
            .unwrap();

        let descriptor_set_layout_size =
            device.get_descriptor_set_layout_size_ext(descriptor_set_layout);

        DescriptorSetLayoutHandle {
            descriptor_set_layout: Some(descriptor_set_layout),
            descriptor_set_layout_size,
        }
    }

    fn get_descriptor_buffer_aligned_size(value: u64, alignment: u64) -> u64 {
        (value + alignment - 1) & !(alignment - 1)
    }
}
