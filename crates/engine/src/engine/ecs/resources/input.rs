use ahash::AHashSet;
//use ahash::{AHashSet, HashSet};
use bevy_ecs::resource::Resource;
use math::Vec2;
use winit::keyboard::KeyCode;

#[derive(Resource)]
pub struct Input {
    pressed: AHashSet<KeyCode>,
    just_pressed: AHashSet<KeyCode>,
    just_released: AHashSet<KeyCode>,
    mouse_delta: Vec2,
    mouse_axis: Vec2,
}

impl Input {
    const CAPACITY: usize = KeyCode::ZoomToggle as usize;

    pub(crate) fn new() -> Self {
        Self {
            pressed: AHashSet::with_capacity(Self::CAPACITY),
            just_pressed: AHashSet::with_capacity(Self::CAPACITY),
            just_released: AHashSet::with_capacity(Self::CAPACITY),
            mouse_delta: Default::default(),
            mouse_axis: Default::default(),
        }
    }

    pub fn pressed(&self, key: KeyCode) -> bool {
        self.pressed.contains(&key)
    }

    pub fn just_pressed(&self, key: KeyCode) -> bool {
        self.just_pressed.contains(&key)
    }

    pub fn get_mouse_delta(&self) -> Vec2 {
        self.mouse_delta
    }

    pub fn get_mouse_axis(&self) -> Vec2 {
        self.mouse_axis
    }

    pub(crate) fn press(&mut self, key: KeyCode) {
        if !self.pressed.contains(&key) {
            self.just_pressed.insert(key);
        }
        self.pressed.insert(key);
    }

    pub(crate) fn release(&mut self, key: KeyCode) {
        self.pressed.remove(&key);
        self.just_released.insert(key);
    }

    pub(crate) fn set_mouse_delta(&mut self, mouse_delta: (f32, f32)) {
        self.mouse_delta = Vec2::new(mouse_delta.0, mouse_delta.1);

        let mouse_delta = Vec2::new(self.mouse_delta.x, -self.mouse_delta.y);
        self.mouse_axis += mouse_delta;
    }

    pub(crate) fn reset(&mut self) {
        self.just_pressed.clear();
        self.just_released.clear();
        self.mouse_axis = Default::default();
    }
}
