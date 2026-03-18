use std::time::Instant;

use bevy_ecs::resource::Resource;

// TODO: Move to the "resources" module.
#[derive(Resource)]
pub struct Time {
    delta_time: f32,
    last_frame: Instant,
}

impl Default for Time {
    fn default() -> Self {
        Self::new()
    }
}

impl Time {
    pub fn new() -> Self {
        Self {
            delta_time: Default::default(),
            last_frame: Instant::now(),
        }
    }

    pub fn get_delta_time(&self) -> f32 {
        self.delta_time
    }

    pub fn update(&mut self) {
        let now = std::time::Instant::now();
        let duration = now.duration_since(self.last_frame);

        self.delta_time = duration.as_secs_f32();
        self.last_frame = now;
    }
}
