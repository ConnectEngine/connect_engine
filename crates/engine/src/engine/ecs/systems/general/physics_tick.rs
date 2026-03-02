use bevy_ecs::system::ResMut;

pub fn physics_tick_system(mut physics: ResMut<physics::Physics>) {
    physics.step();
}
