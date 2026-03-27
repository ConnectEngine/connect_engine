use bevy_ecs::system::{Res, ResMut};
use connect_renderer::*;
use vulkanite::vk::*;

pub fn prepare_default_samplers_system(
    mut renderer_resources: ResMut<RendererResources>,
    mut descriptor_set_handle: ResMut<DescriptorSetHandle>,
    buffers_pool: Res<BuffersPool>,
    mut samplers_pool: ResMut<SamplersPool>,
) {
    let default_sampler_reference =
        samplers_pool.create_sampler(Filter::Linear, SamplerAddressMode::Repeat, true);
    renderer_resources.default_sampler_reference = default_sampler_reference;

    let sampler = samplers_pool
        .get_sampler(default_sampler_reference)
        .unwrap();
    let sampler_descriptor = DescriptorKind::Sampler(DescriptorSampler {
        sampler: *sampler,
        index: renderer_resources.default_sampler_reference.get_index(),
    });

    descriptor_set_handle.update_binding(&buffers_pool, sampler_descriptor);
}
