mod ecs;
mod setup;
mod utils;

use connect_renderer::*;
use ecs::*;

use bevy_ecs::{
    entity_disabling::Disabled,
    schedule::{IntoScheduleConfigs, ScheduleLabel, Schedules},
    world::World,
};
use connect_importer::Importer;
use connect_information::*;
use connect_loader::Loader;
use connect_math::Random;
use vulkan::vk::{
    DeviceV1_0, ExtDebugUtilsExtensionInstanceCommands, ExtShaderObjectExtensionDeviceCommands,
    InstanceV1_0, KhrSurfaceExtensionInstanceCommands, KhrSwapchainExtensionDeviceCommands,
};
use winit::{event::ElementState, keyboard::KeyCode, window::Window};

use crate::{
    GamePlugin,
    engine::ecs::{
        general::{
            check_audio_state, physics_tick, propogate_disabled_to_new_children, update_time,
        },
        setup::{
            prepare_default_samplers::prepare_default_samplers_system,
            prepare_default_textures::prepare_default_textures_system,
            prepare_shaders::prepare_shaders_system,
        },
    },
};

pub use audio::*;
pub use components::camera::{Camera, ClippingPlanes};
pub use components::time::Time;
// TODO: Later make unified access to the assets (for example, `Assets` System Param or something like that).
pub use connect_asset_database::*;
pub use connect_loader::events::LoadModelEvent;
pub use connect_math;
pub use connect_shared::LocalTransform;
pub use input::*;
pub use physics::{Collider, RigidBody};
pub use queries::transform::*;
pub use system_params::physics::*;

#[derive(Clone, Copy, PartialEq, Eq, Hash, ScheduleLabel, Debug)]
struct SchedulerWorldUpdate;

#[derive(Clone, Copy, PartialEq, Eq, Hash, ScheduleLabel, Debug)]
struct SchedulerRendererSetup;

#[derive(Clone, Copy, PartialEq, Eq, Hash, ScheduleLabel, Debug)]
struct SchedulerEngineStartup;

#[derive(Clone, Copy, PartialEq, Eq, Hash, ScheduleLabel, Debug)]
struct SchedulerRendererUpdate;

#[derive(Clone, Copy, PartialEq, Eq, Hash, ScheduleLabel, Debug)]
pub struct SchedulerGameInit;

#[derive(Clone, Copy, PartialEq, Eq, Hash, ScheduleLabel, Debug)]
pub struct SchedulerGameUpdate;

pub struct Engine {
    world: World,
}

impl Engine {
    pub fn new(window: &dyn Window) -> Self {
        let mut world: World = World::new();
        world.register_disabling_component::<Disabled>();

        let vulkan_context_resource = Self::create_vulkan_context(window);
        world.insert_resource(vulkan_context_resource);

        let device_properties_resource = Self::create_device_properties(&world);
        world.insert_resource(device_properties_resource);

        let render_context = Self::create_renderer_context(window, &world);
        world.insert_resource(render_context);

        Self::prepare_renderer_resources(&mut world);

        let frame_context = FrameContextResource::default();
        world.insert_resource(frame_context);

        world.init_resource::<Schedules>();
        world.insert_resource(Information::new());
        world.insert_resource(AssetDatabase::new());
        world.insert_resource(Importer::new());
        world.insert_resource(Loader::new());

        let mut schedulers = world.resource_mut::<Schedules>();

        let scheduler_engine_startup = schedulers.entry(SchedulerEngineStartup);
        scheduler_engine_startup.add_systems(
            (
                connect_importer::collect_assets_to_serialize_system,
                connect_importer::resolve_assets_entries_system,
                connect_importer::check_if_asset_is_serialized_system,
                connect_importer::serialize_unserialized_assets_system,
                connect_loader::load_assets_system,
            )
                .chain(),
        );

        let scheduler_world_update = schedulers.entry(SchedulerWorldUpdate);
        scheduler_world_update.add_systems(
            (
                propogate_disabled_to_new_children::propagate_disabled_to_new_children_system,
                physics_tick::physics_tick_system,
                physics_tick::physics_update_global_transforms,
                physics_tick::physics_update_local_transforms,
                propogate_transforms_system,
            )
                .chain(),
        );

        scheduler_world_update.add_systems(update_time::update_time_system);

        let scheduler_renderer_setup = schedulers.entry(SchedulerRendererSetup);
        scheduler_renderer_setup.add_systems(
            (
                prepare_default_samplers_system,
                prepare_default_textures_system,
                prepare_shaders_system,
            )
                .chain(),
        );

        let scheduler_renderer_update = schedulers.entry(SchedulerRendererUpdate);
        scheduler_renderer_update.add_systems(
            (
                check_audio_state::check_audio_state_system,
                prepare_frame::prepare_frame_system,
                collect_instance_objects::collect_instance_objects_system,
                update_resources::update_resources_system,
                begin_rendering::begin_rendering_system,
                render_meshes::render_meshes_system,
                end_rendering::end_rendering_system,
                present::present_system,
            )
                .chain(),
        );

        schedulers.entry(SchedulerGameInit);
        schedulers.entry(SchedulerGameUpdate);

        world.add_observer(on_spawn_model::on_spawn_mesh_system);

        world.insert_resource(Time::new());
        world.insert_resource(Input::new());
        world.insert_resource(Random::new());
        world.insert_resource(physics::PhysicsManager::new());

        world.run_schedule(SchedulerRendererSetup);
        world.run_schedule(SchedulerEngineStartup);
        world.flush();

        // TODO: In future, we need to fix this. Awful code.
        let mut exe_path = std::env::current_exe().unwrap();

        exe_path.pop();
        exe_path.pop();
        exe_path.pop();

        Self { world }
    }

    pub fn init_game(&mut self, game_plugin: &dyn GamePlugin) {
        let mut schedules = self.world.resource_mut::<Schedules>();

        game_plugin.add_systems_init(schedules.get_mut(SchedulerGameInit).unwrap());
        game_plugin.add_systems_update(schedules.get_mut(SchedulerGameUpdate).unwrap());

        self.world.run_schedule(SchedulerGameInit);
    }

    pub fn update(&mut self) {
        self.world.run_schedule(SchedulerWorldUpdate);
        self.world.run_schedule(SchedulerGameUpdate);
        self.world.flush();

        self.world.run_schedule(SchedulerRendererUpdate);

        let mut input = unsafe { self.world.get_resource_mut::<Input>().unwrap_unchecked() };
        input.reset();
    }

    pub fn process_input(&mut self, key_code: KeyCode, state: ElementState) {
        let mut input = unsafe { self.world.get_resource_mut::<Input>().unwrap_unchecked() };
        if state == ElementState::Pressed {
            input.press(key_code);
        } else {
            input.release(key_code);
        }
    }

    pub fn process_mouse(&mut self, mouse_delta: (f32, f32)) {
        let mut input = unsafe { self.world.get_resource_mut::<Input>().unwrap_unchecked() };
        input.set_mouse_delta(mouse_delta);
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        let mut vulkan_context_resource = self
            .world
            .remove_resource::<VulkanContextResource>()
            .unwrap();
        let mut render_context_resource = self
            .world
            .remove_resource::<RendererContextResource>()
            .unwrap();
        let mut buffers_pool = self.world.remove_resource::<BuffersPoolResource>().unwrap();
        let mut textures_pool = self
            .world
            .remove_resource::<TexturesPoolResource>()
            .unwrap();
        let mut samplers_pool = self.world.remove_resource::<SamplersPool>().unwrap();
        let renderer_resources = self.world.remove_resource::<RendererResources>().unwrap();
        let descriptor_set_handle = self.world.remove_resource::<DescriptorSetHandle>().unwrap();

        let device = vulkan_context_resource.device.as_ref();

        unsafe {
            device.device_wait_idle().unwrap();
        }

        unsafe {
            buffers_pool.free_allocations();
            textures_pool.free_allocations();
            samplers_pool.destroy_samplers();
            descriptor_set_handle.destroy();

            device.destroy_shader_ext(
                renderer_resources.gradient_compute_shader_object.shader,
                None,
            );
            device.destroy_shader_ext(renderer_resources.mesh_shader_object.shader, None);
            device.destroy_shader_ext(renderer_resources.task_shader_object.shader, None);
            device.destroy_shader_ext(renderer_resources.fragment_shader_object.shader, None);

            device.destroy_command_pool(
                render_context_resource
                    .upload_context
                    .command_group
                    .command_pool,
                None,
            );
            device.destroy_fence(
                render_context_resource.upload_context.command_group.fence,
                None,
            );

            render_context_resource
                .frames_data
                .iter()
                .for_each(|frame_data| {
                    device.destroy_command_pool(frame_data.command_group.command_pool, None);
                    device.destroy_fence(frame_data.command_group.fence, None);
                    device.destroy_semaphore(frame_data.render_semaphore, None);
                    device.destroy_semaphore(frame_data.swapchain_semaphore, None);
                });

            render_context_resource
                .image_views
                .iter()
                .for_each(|&image_view| {
                    device.destroy_image_view(image_view, None);
                });

            if let Some(debug_utils_messenger) = vulkan_context_resource.debug_utils_messenger {
                vulkan_context_resource
                    .instance
                    .destroy_debug_utils_messenger_ext(debug_utils_messenger, None);
            }

            vulkan_context_resource
                .device
                .destroy_swapchain_khr(vulkan_context_resource.swapchain, None);
            vulkan_context_resource.device.destroy_device(None);
            vulkan_context_resource
                .instance
                .destroy_surface_khr(vulkan_context_resource.surface, None);
            vulkan_context_resource.instance.destroy_instance(None);
        }
    }
}
