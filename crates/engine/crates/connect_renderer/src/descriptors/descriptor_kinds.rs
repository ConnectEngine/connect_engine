use vulkanite::vk::rs::*;

#[derive(Clone, Copy)]
pub struct DescriptorStorageImage {
    pub image_view: ImageView,
    pub index: u32,
}

#[derive(Clone, Copy)]
pub struct DescriptorSampledImage {
    pub image_view: ImageView,
    pub index: u32,
}

#[derive(Clone, Copy)]
pub struct DescriptorSampler {
    pub sampler: Sampler,
    pub index: u32,
}
