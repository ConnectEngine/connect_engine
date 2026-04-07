use std::sync::Arc;

use bevy_ecs::resource::Resource;
use connect_shared::{TextureExtent, TextureFormat, TextureKey, TextureMetadata, TextureType};
use slotmap::{Key, SlotMap};
use vulkan::{Device, vk::*};
use vulkan_vma::*;

pub struct AllocatedImage {
    pub image: vulkan::vk::Image,
    pub image_view: vulkan::vk::ImageView,
    pub allocation: Allocation,
    pub extent: Extent3D,
    pub image_aspect_flags: ImageAspectFlags,
    pub format: Format,
    pub subresource_range: ImageSubresourceRange,
    pub texture_metadata: TextureMetadata,
}

#[derive(Default, Clone, Copy, Hash, PartialEq, Eq)]
pub struct TextureReference {
    pub key: TextureKey,
    pub texture_metadata: TextureMetadata,
    read_only: bool,
}

impl TextureReference {
    pub fn get_index(&self) -> u32 {
        self.key.data().get_key() - 1
    }
}

#[derive(Resource)]
pub struct TexturesPoolResource {
    device: Arc<Device>,
    allocator: Allocator,
    storage_slots: SlotMap<TextureKey, AllocatedImage>,
    sampled_slots: SlotMap<TextureKey, AllocatedImage>,
}

impl TexturesPoolResource {
    pub fn new(device: Arc<Device>, allocator: Allocator) -> Self {
        Self {
            device,
            allocator,
            storage_slots: SlotMap::with_capacity_and_key(128),
            sampled_slots: SlotMap::with_capacity_and_key(10_000),
        }
    }

    pub fn create_texture(
        &mut self,
        texture_type: TextureType,
        format: Format,
        extent: Extent3D,
        usage_flags: ImageUsageFlags,
        mip_map_enabled: bool,
        layers_count: u32,
    ) -> TextureReference {
        let read_only = usage_flags.contains(ImageUsageFlags::SAMPLED);

        let mut aspect_flags = ImageAspectFlags::COLOR;
        if format == Format::D32_SFLOAT {
            aspect_flags = ImageAspectFlags::DEPTH;
        }

        let mip_levels_count = if mip_map_enabled {
            f32::max(extent.width as _, extent.height as _)
                .log2()
                .floor() as u32
                + 1
        } else {
            1
        };

        let texture_metadata = TextureMetadata {
            texture_type,
            texture_format: TextureFormat::try_from(format).unwrap(),
            texture_extent: TextureExtent {
                width: extent.width,
                height: extent.height,
                depth: extent.depth,
            },
            mip_levels_count,
            layers_count,
        };

        let texture_reference = self.upload_texture(
            texture_metadata,
            extent,
            usage_flags,
            aspect_flags,
            read_only,
        );

        texture_reference
    }

    #[must_use]
    pub fn upload_texture(
        &mut self,
        texture_metadata: TextureMetadata,
        extent: Extent3D,
        usage_flags: ImageUsageFlags,
        aspect_flags: ImageAspectFlags,
        read_only: bool,
    ) -> TextureReference {
        let allocation_info = AllocationOptions {
            usage: MemoryUsage::Auto,
            required_flags: MemoryPropertyFlags::DEVICE_LOCAL,
            ..Default::default()
        };

        let format: Format = texture_metadata.texture_format.try_into().unwrap();
        let image_create_info = Self::get_image_info(
            format,
            usage_flags,
            extent,
            ImageLayout::UNDEFINED,
            texture_metadata.mip_levels_count,
        );
        let (allocated_image, allocation) = unsafe {
            self.allocator
                .create_image(image_create_info, &allocation_info)
                .unwrap()
        };

        let image_view_create_info = Self::get_image_view_info(
            format,
            allocated_image,
            aspect_flags,
            texture_metadata.mip_levels_count,
        );
        let image_view = unsafe {
            self.device
                .create_image_view(&image_view_create_info, None)
                .unwrap()
        };

        let allocated_image = AllocatedImage {
            image: allocated_image,
            image_view,
            allocation,
            extent,
            format,
            image_aspect_flags: aspect_flags,
            subresource_range: image_view_create_info.subresource_range,
            texture_metadata,
        };

        self.insert_image(allocated_image, read_only)
    }

    fn insert_image(
        &mut self,
        allocated_image: AllocatedImage,
        read_only: bool,
    ) -> TextureReference {
        let texture_key;
        let texture_metadata: TextureMetadata;

        match read_only {
            true => {
                texture_metadata = allocated_image.texture_metadata;
                texture_key = self.sampled_slots.insert(allocated_image);
            }
            false => {
                texture_metadata = allocated_image.texture_metadata;
                texture_key = self.storage_slots.insert(allocated_image);
            }
        }

        TextureReference {
            key: texture_key,
            texture_metadata,
            read_only,
        }
    }

    fn is_compressed_image_format(format: Format) -> bool {
        matches!(
            format,
            Format::BC1_RGB_SRGB_BLOCK
                | Format::BC3_SRGB_BLOCK
                | Format::BC4_UNORM_BLOCK
                | Format::BC5_UNORM_BLOCK
                | Format::BC6H_SFLOAT_BLOCK
                | Format::BC7_SRGB_BLOCK
        )
    }

    pub fn get_image(&self, texture_reference: TextureReference) -> Option<&AllocatedImage> {
        let allocated_image;

        if texture_reference.read_only {
            allocated_image = self.sampled_slots.get(texture_reference.key);
        } else {
            allocated_image = self.storage_slots.get(texture_reference.key);
        }

        allocated_image
    }

    pub fn get_image_info<'a>(
        format: Format,
        usage_flags: ImageUsageFlags,
        extent: Extent3D,
        initial_layout: ImageLayout,
        mip_levels: u32,
    ) -> ImageCreateInfo {
        ImageCreateInfoBuilder::default()
            .image_type(ImageType::_2D)
            .format(format)
            .extent(extent)
            .mip_levels(mip_levels)
            .array_layers(1)
            .samples(SampleCountFlags::_1)
            .tiling(ImageTiling::OPTIMAL)
            .usage(usage_flags)
            .sharing_mode(SharingMode::EXCLUSIVE)
            .initial_layout(initial_layout)
            .build()
    }

    pub fn get_image_view_info(
        format: Format,
        image: vulkan::vk::Image,
        image_aspect_flags: ImageAspectFlags,
        level_count: u32,
    ) -> ImageViewCreateInfo {
        let mut image_view_create_info = ImageViewCreateInfoBuilder::default()
            .view_type(ImageViewType::_2D)
            .format(format)
            .components(ComponentMapping {
                r: ComponentSwizzle::R,
                g: ComponentSwizzle::G,
                b: ComponentSwizzle::B,
                a: ComponentSwizzle::A,
            })
            .subresource_range(
                ImageSubresourceRangeBuilder::default()
                    .aspect_mask(image_aspect_flags)
                    .base_mip_level(Default::default())
                    .level_count(level_count)
                    .base_array_layer(Default::default())
                    .layer_count(1)
                    .build(),
            );
        let image_view_create_info = image_view_create_info.image(image).build();

        image_view_create_info
    }

    pub fn free_allocations(&mut self) {
        self.sampled_slots
            .iter_mut()
            .for_each(|(_, allocated_image)| unsafe {
                self.device
                    .destroy_image_view(allocated_image.image_view, None);
                self.allocator
                    .destroy_image(allocated_image.image, allocated_image.allocation);
            });

        self.storage_slots
            .iter_mut()
            .for_each(|(_, allocated_image)| unsafe {
                self.device
                    .destroy_image_view(allocated_image.image_view, None);
                self.allocator
                    .destroy_image(allocated_image.image, allocated_image.allocation);
            });
    }
}
