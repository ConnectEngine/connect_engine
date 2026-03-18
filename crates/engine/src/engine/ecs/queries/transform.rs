use bevy_ecs::query::QueryData;
use math::{EulerRot, Mat4, Quat, Vec3};

use crate::engine::{LocalTransform, ecs::components::local_transform::GlobalTransform};

#[derive(QueryData)]
#[query_data(mutable)]
pub struct Transform {
    local: &'static mut LocalTransform,
    global: &'static GlobalTransform,
}

impl<'w, 's> TransformItem<'w, 's> {
    pub fn scale(&self) -> Vec3 {
        self.global.0.to_scale_rotation_translation().0
    }

    pub fn rotation(&self) -> Quat {
        self.global.0.to_scale_rotation_translation().1
    }

    pub fn position(&self) -> Vec3 {
        self.global.0.to_scale_rotation_translation().2
    }

    pub fn get_local_position(&self) -> Vec3 {
        self.local.local_position
    }

    pub fn set_local_position(&mut self, pos: Vec3) {
        self.local.local_position = pos;
    }

    pub fn get_local_rotation(&self) -> Quat {
        self.local.local_rotation
    }

    pub fn set_local_rotation(&mut self, rot: Quat) {
        self.local.local_rotation = rot;
    }

    pub fn get_local_euler_angles(&self) -> Vec3 {
        let (y, x, z) = self.local.local_rotation.to_euler(EulerRot::YXZ);
        Vec3::new(x.to_degrees(), y.to_degrees(), z.to_degrees())
    }

    pub fn set_local_euler_angles(&mut self, euler_degrees: Vec3) {
        let x_rad = euler_degrees.x.to_radians();
        let y_rad = euler_degrees.y.to_radians();
        let z_rad = euler_degrees.z.to_radians();

        self.local.local_rotation = Quat::from_euler(EulerRot::YXZ, y_rad, x_rad, z_rad);
    }

    pub fn forward(&self) -> Vec3 {
        let mut forward = self.local.local_rotation * Vec3::NEG_Z;
        forward.y = Default::default();

        forward
    }

    pub fn right(&self) -> Vec3 {
        let mut right = self.local.local_rotation * Vec3::X;
        right.y = Default::default();

        right
    }

    pub fn up(&self) -> Vec3 {
        self.local.local_rotation * Vec3::Y
    }

    pub fn translate_local(&mut self, translation: Vec3) {
        let local_rotation = self.local.local_rotation;

        self.local.local_position += local_rotation * translation;
    }

    pub fn look_at(&mut self, target: Vec3, world_up: Vec3) {
        let forward = (target - self.local.local_position).normalize_or_zero();
        if forward == Vec3::ZERO {
            return;
        }

        let rotation_matrix = Mat4::look_at_rh(Vec3::ZERO, forward, world_up).inverse();
        self.local.local_rotation = Quat::from_mat4(&rotation_matrix);
    }

    pub fn local_to_world_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            self.local.local_scale,
            self.local.local_rotation,
            self.local.local_position,
        )
    }
}

impl<'w, 's> TransformReadOnlyItem<'w, 's> {
    pub fn scale(&self) -> Vec3 {
        self.global.0.to_scale_rotation_translation().0
    }

    pub fn rotation(&self) -> Quat {
        self.global.0.to_scale_rotation_translation().1
    }

    pub fn position(&self) -> Vec3 {
        self.global.0.to_scale_rotation_translation().2
    }

    pub fn get_local_position(&self) -> Vec3 {
        self.local.local_position
    }

    pub fn get_local_rotation(&self) -> Quat {
        self.local.local_rotation
    }

    pub fn get_local_euler_angles(&self) -> Vec3 {
        let (y, x, z) = self.local.local_rotation.to_euler(EulerRot::YXZ);
        Vec3::new(x.to_degrees(), y.to_degrees(), z.to_degrees())
    }

    pub fn forward(&self) -> Vec3 {
        let mut forward = self.local.local_rotation * Vec3::NEG_Z;
        forward.y = Default::default();

        forward
    }

    pub fn right(&self) -> Vec3 {
        let mut right = self.local.local_rotation * Vec3::X;
        right.y = Default::default();

        right
    }

    pub fn up(&self) -> Vec3 {
        self.local.local_rotation * Vec3::Y
    }

    pub fn local_to_world_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            self.local.local_scale,
            self.local.local_rotation,
            self.local.local_position,
        )
    }
}
