use std::path::PathBuf;

use bevy_ecs::component::Component;

#[derive(Component)]
pub struct ModelTag {
    pub path_buf: PathBuf,
}
