use bevy_ecs::system::{Res, ResMut};
use connect_math::*;
use connect_renderer::*;

use vulkan::vk::*;

pub fn prepare_default_textures_system(
    vulkan_ctx_resource: Res<VulkanContextResource>,
    mut renderer_context: ResMut<RendererContextResource>,
    mut renderer_resources: ResMut<RendererResources>,
    mut descriptor_set_handle: ResMut<DescriptorSetHandle>,
    mut textures_pool: ResMut<TexturesPoolResource>,
    mut buffers_pool: ResMut<BuffersPoolResource>,
) {
    let magenta = &pack_unorm_4x8(Vec4::new(1.0, 0.0, 1.0, 1.0));
    let black = &pack_unorm_4x8(Vec4::new(0.0, 0.0, 0.0, 0.0));
    let mut pixels: Vec<u32> = vec![0; 16 * 16];
    for x in 0..16 {
        for y in 0..16 {
            pixels[y * 16 + x] = if (x % 2) ^ (y % 2) == 0 {
                *magenta
            } else {
                *black
            };
        }
    }

    let checkerboard_image_extent = Extent3D {
        width: 16,
        height: 16,
        depth: 1,
    };
    let (checkerboard_texture_reference, _) = textures_pool.create_texture(
        None,
        false,
        Format::R8G8B8A8_UNORM,
        checkerboard_image_extent,
        ImageUsageFlags::SAMPLED | ImageUsageFlags::TRANSFER_DST,
        false,
    );

    renderer_resources.default_texture_reference = checkerboard_texture_reference;
    let descriptor_checkerboard_image = DescriptorKind::SampledImage(DescriptorSampledImage {
        image_view: textures_pool
            .get_image(checkerboard_texture_reference)
            .unwrap()
            .image_view,
        index: checkerboard_texture_reference.get_index(),
    });
    descriptor_set_handle.update_binding(&buffers_pool, descriptor_checkerboard_image);

    vulkan_ctx_resource.transfer_data_to_image(
        textures_pool
            .get_image(checkerboard_texture_reference)
            .unwrap(),
        &mut buffers_pool,
        pixels.as_ptr() as *const _,
        &renderer_context.upload_context,
        None,
    );

    let white_image_extent = Extent3D {
        width: 1,
        height: 1,
        depth: 1,
    };
    let (white_texture_reference, _) = textures_pool.create_texture(
        None,
        false,
        Format::R8G8B8A8_SRGB,
        white_image_extent,
        ImageUsageFlags::SAMPLED | ImageUsageFlags::TRANSFER_DST,
        false,
    );
    renderer_resources.fallback_texture_reference = white_texture_reference;

    let white_image_pixels = [pack_unorm_4x8(Vec4::new(1.0, 1.0, 1.0, 1.0))];
    vulkan_ctx_resource.transfer_data_to_image(
        textures_pool.get_image(white_texture_reference).unwrap(),
        &mut buffers_pool,
        white_image_pixels.as_ptr() as *const _,
        &renderer_context.upload_context,
        None,
    );

    let descriptor_white_image = DescriptorKind::SampledImage(DescriptorSampledImage {
        image_view: textures_pool
            .get_image(white_texture_reference)
            .unwrap()
            .image_view,
        index: white_texture_reference.get_index(),
    });
    descriptor_set_handle.update_binding(&buffers_pool, descriptor_white_image);

    let draw_extent = renderer_context.draw_extent;
    renderer_context
        .frames_data
        .iter_mut()
        .for_each(|frame_data| {
            let draw_image_extent = Extent3D {
                width: draw_extent.width,
                height: draw_extent.height,
                depth: 1,
            };

            let (draw_texture_reference, _) = textures_pool.create_texture(
                None,
                false,
                Format::R16G16B16A16_SFLOAT,
                draw_image_extent,
                ImageUsageFlags::TRANSFER_SRC
                    | ImageUsageFlags::STORAGE
                    | ImageUsageFlags::COLOR_ATTACHMENT,
                false,
            );

            let (depth_texture_reference, _) = textures_pool.create_texture(
                None,
                false,
                Format::D32_SFLOAT,
                draw_image_extent,
                ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
                false,
            );

            let descriptor_draw_image = DescriptorKind::StorageImage(DescriptorStorageImage {
                image_view: textures_pool
                    .get_image(draw_texture_reference)
                    .unwrap()
                    .image_view,
                index: draw_texture_reference.get_index(),
            });
            descriptor_set_handle.update_binding(&buffers_pool, descriptor_draw_image);

            frame_data.draw_texture_reference = draw_texture_reference;
            frame_data.depth_texture_reference = depth_texture_reference;
        });
}

pub fn pack_unorm_4x8(v: Vec4) -> u32 {
    let v = v.clamp(Vec4::ZERO, Vec4::ONE) * 255.0;

    // 3. Round to nearest integer and cast to u8
    // Note: using arrays + map is often cleaner than manual bit shifting
    let [x, y, z, w] = v.to_array().map(|c| c.round() as u8);

    // 4. Pack into u32 using Little Endian (x is LSB, w is MSB)
    // This matches the GLSL behavior:
    // Bits 0-7:   x
    // Bits 8-15:  y
    // Bits 16-23: z
    // Bits 24-31: w
    u32::from_le_bytes([x, y, z, w])
}
