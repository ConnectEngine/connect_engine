use bevy_ecs::resource::Resource;
use bytemuck::{Pod, Zeroable};
use fast_image_resize::{PixelType, images::Image};
use ktx2_rw::{BasisCompressionParams, Ktx2Texture};
use shared::TextureKey;
use slotmap::{Key, SlotMap};
use vma::{Alloc, Allocation, AllocationCreateInfo, Allocator, MemoryUsage};
use vulkanite::vk::{
    ComponentMapping, ComponentSwizzle, Extent3D, Format, ImageAspectFlags, ImageCreateInfo,
    ImageLayout, ImageSubresourceRange, ImageTiling, ImageType, ImageUsageFlags,
    ImageViewCreateInfo, ImageViewType, MemoryPropertyFlags, SampleCountFlags, SharingMode,
    rs::Device,
};

#[repr(C)]
#[derive(Default, Clone, Copy, Pod, Zeroable)]
pub struct TextureMetadata {
    pub width: u32,
    pub height: u32,
    pub mip_levels_count: u32,
}

pub struct AllocatedImage {
    pub image: vulkanite::vk::rs::Image,
    pub image_view: vulkanite::vk::rs::ImageView,
    pub allocation: Allocation,
    pub extent: Extent3D,
    pub image_aspect_flags: ImageAspectFlags,
    pub format: Format,
    pub subresource_range: ImageSubresourceRange,
    pub texture_metadata: TextureMetadata,
}

#[derive(Default, Clone, Copy)]
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
pub struct TexturesPool {
    device: Device,
    allocator: Allocator,
    storage_slots: SlotMap<TextureKey, AllocatedImage>,
    sampled_slots: SlotMap<TextureKey, AllocatedImage>,
}

impl TexturesPool {
    pub fn new(device: Device, allocator: Allocator) -> Self {
        Self {
            device,
            allocator,
            storage_slots: SlotMap::with_capacity_and_key(128),
            sampled_slots: SlotMap::with_capacity_and_key(10_000),
        }
    }

    pub fn create_texture(
        &mut self,
        data: Option<&mut [u8]>,
        is_cached: bool,
        format: Format,
        extent: Extent3D,
        usage_flags: ImageUsageFlags,
        mip_map_enabled: bool,
    ) -> (TextureReference, Option<Ktx2Texture>) {
        let read_only = usage_flags.contains(ImageUsageFlags::Sampled);

        let mut aspect_flags = ImageAspectFlags::Color;
        if format == Format::D32Sfloat {
            aspect_flags = ImageAspectFlags::Depth;
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
            width: extent.width,
            height: extent.height,
            mip_levels_count,
        };

        let mut ktx_texture = None;
        if Self::is_compressed_image_format(format)
            // TODO: Make it more flexible and less error prone.
            && !is_cached
            && let Some(data) = data
        {
            let target_ktx_format = match format {
                Format::Bc3SrgbBlock | Format::Bc1RgbSrgbBlock => ktx2_rw::VkFormat::R8G8B8A8_SRGB,
                _ => panic!("Unsupported KTX format: {:?}!", format),
            };

            let mut texture = Ktx2Texture::create(
                texture_metadata.width,
                texture_metadata.height,
                1,
                1,
                1,
                mip_levels_count,
                target_ktx_format,
            )
            .unwrap();

            let src_image = match format {
                Format::Bc3SrgbBlock => Image::from_slice_u8(
                    texture_metadata.width,
                    texture_metadata.height,
                    data,
                    PixelType::U8x4,
                )
                .unwrap(),
                Format::Bc1RgbSrgbBlock => Image::from_slice_u8(
                    texture_metadata.width,
                    texture_metadata.height,
                    data,
                    PixelType::U8x4,
                )
                .unwrap(),
                _ => panic!("Unsupported Image format: {:?}!", format),
            };

            // TODO: We can effectively pre-allocate required total size of texture_data
            let mut texture_data = Vec::new();
            for mip_level_index in 0..mip_levels_count {
                let current_width = (texture_metadata.width >> mip_level_index).max(1);
                let current_height = (texture_metadata.height >> mip_level_index).max(1);

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

                texture
                    .set_image_data(mip_level_index, 0, 0, image_bytes)
                    .unwrap();
            }

            texture
                .compress_basis(
                    &BasisCompressionParams::builder()
                        .thread_count((num_cpus::get() - 1) as _)
                        .build(),
                )
                .unwrap();

            let transcode_format = match format {
                Format::Bc1RgbSrgbBlock => ktx2_rw::TranscodeFormat::Bc1Rgb,
                Format::Bc3SrgbBlock => ktx2_rw::TranscodeFormat::Bc3Rgba,
                Format::Bc7SrgbBlock => ktx2_rw::TranscodeFormat::Bc7Rgba,
                _ => panic!("Unsupported transcode format!"),
            };

            texture.transcode_basis(transcode_format).unwrap();

            for mip_level_index in 0..mip_levels_count {
                let texture_data_ref = texture.get_image_data(mip_level_index, 0, 0).unwrap();
                texture_data.extend_from_slice(texture_data_ref);
            }

            texture
                .set_metadata(
                    stringify!(TextureMetadata),
                    bytemuck::bytes_of(&texture_metadata),
                )
                .unwrap();

            ktx_texture = Some(texture);
        }

        let allocation_info = AllocationCreateInfo {
            usage: MemoryUsage::Auto,
            required_flags: MemoryPropertyFlags::DeviceLocal,
            ..Default::default()
        };

        let image_create_info = Self::get_image_info(
            format,
            usage_flags,
            extent,
            ImageLayout::Undefined,
            mip_levels_count,
        );
        let (allocated_image, allocation) = unsafe {
            self.allocator
                .create_image(&image_create_info, &allocation_info)
                .unwrap()
        };

        let image = vulkanite::vk::rs::Image::from_inner(allocated_image);
        let image_view_create_info =
            Self::get_image_view_info(format, &image, aspect_flags, mip_levels_count);
        let image_view = self
            .device
            .create_image_view(&image_view_create_info)
            .unwrap();

        let allocated_image = AllocatedImage {
            image,
            image_view,
            allocation,
            extent,
            format,
            image_aspect_flags: aspect_flags,
            subresource_range: image_view_create_info.subresource_range,
            texture_metadata: TextureMetadata {
                width: extent.width,
                height: extent.height,
                mip_levels_count,
            },
        };

        (self.insert_image(allocated_image, read_only), ktx_texture)
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
            Format::Bc1RgbSrgbBlock
                | Format::Bc3SrgbBlock
                | Format::Bc4SnormBlock
                | Format::Bc5SnormBlock
                | Format::Bc6HSfloatBlock
                | Format::Bc7SrgbBlock
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
    ) -> ImageCreateInfo<'a> {
        ImageCreateInfo::default()
            .image_type(ImageType::Type2D)
            .format(format)
            .extent(extent)
            .mip_levels(mip_levels)
            .array_layers(1)
            .samples(SampleCountFlags::Count1)
            .tiling(ImageTiling::Optimal)
            .usage(usage_flags)
            .sharing_mode(SharingMode::Exclusive)
            .initial_layout(initial_layout)
    }

    pub fn get_image_view_info<'a>(
        format: Format,
        image: &'a vulkanite::vk::rs::Image,
        image_aspect_flags: ImageAspectFlags,
        level_count: u32,
    ) -> ImageViewCreateInfo<'a> {
        let mut image_view_create_info = ImageViewCreateInfo::default()
            .view_type(ImageViewType::Type2D)
            .format(format)
            .components(ComponentMapping {
                r: ComponentSwizzle::R,
                g: ComponentSwizzle::G,
                b: ComponentSwizzle::B,
                a: ComponentSwizzle::A,
            })
            .subresource_range(
                ImageSubresourceRange::default()
                    .aspect_mask(image_aspect_flags)
                    .base_mip_level(Default::default())
                    .level_count(level_count)
                    .base_array_layer(Default::default())
                    .layer_count(1),
            );
        image_view_create_info = image_view_create_info.image(image);

        image_view_create_info
    }

    pub fn free_allocations(&mut self) {
        self.sampled_slots
            .iter_mut()
            .for_each(|(_, allocated_image)| unsafe {
                self.device
                    .destroy_image_view(Some(allocated_image.image_view));
                self.allocator
                    .destroy_image(*allocated_image.image, &mut allocated_image.allocation);
            });

        self.storage_slots
            .iter_mut()
            .for_each(|(_, allocated_image)| unsafe {
                self.device
                    .destroy_image_view(Some(allocated_image.image_view));
                self.allocator
                    .destroy_image(*allocated_image.image, &mut allocated_image.allocation);
            });
    }
}
