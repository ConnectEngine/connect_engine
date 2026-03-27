use bevy_ecs::schedule::Schedule;

pub mod engine;
pub use connect_math;

pub trait GamePlugin {
    fn add_systems_init(&self, schedule: &mut Schedule);
    fn add_systems_update(&self, schedule: &mut Schedule);
}
