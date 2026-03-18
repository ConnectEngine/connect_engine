use bevy_ecs::{component::Component, name::Name};
use math::{EulerRot, Mat4, Quat, Vec3};

#[derive(Clone, Copy, Component, Debug)]
#[require(GlobalTransform, Name)]
pub struct LocalTransform {
    pub local_position: Vec3,
    pub local_rotation: Quat,
    pub local_scale: Vec3,
}

impl LocalTransform {
    pub const IDENTITY: LocalTransform = LocalTransform {
        local_position: Vec3::ZERO,
        local_rotation: Quat::IDENTITY,
        local_scale: Vec3::ONE,
    };

    pub fn new(position: Vec3, rotation: Quat, scale: Vec3) -> Self {
        Self {
            local_position: position,
            local_rotation: rotation,
            local_scale: scale,
        }
    }

    pub fn get_local_position(&self) -> Vec3 {
        self.local_position
    }

    pub fn set_local_position(&mut self, pos: Vec3) {
        self.local_position = pos;
    }

    pub fn get_local_rotation(&self) -> Quat {
        self.local_rotation
    }

    pub fn set_local_rotation(&mut self, rot: Quat) {
        self.local_rotation = rot;
    }

    pub fn get_local_euler_angles(&self) -> Vec3 {
        let (y, x, z) = self.local_rotation.to_euler(EulerRot::YXZ);
        Vec3::new(x.to_degrees(), y.to_degrees(), z.to_degrees())
    }

    pub fn set_local_euler_angles(&mut self, euler_degrees: Vec3) {
        let x_rad = euler_degrees.x.to_radians();
        let y_rad = euler_degrees.y.to_radians();
        let z_rad = euler_degrees.z.to_radians();

        self.local_rotation = Quat::from_euler(EulerRot::YXZ, y_rad, x_rad, z_rad);
    }

    pub fn forward(&self) -> Vec3 {
        let mut forward = self.local_rotation * Vec3::NEG_Z;
        forward.y = Default::default();

        forward
    }

    pub fn right(&self) -> Vec3 {
        let mut right = self.local_rotation * Vec3::X;
        right.y = Default::default();

        right
    }

    pub fn up(&self) -> Vec3 {
        self.local_rotation * Vec3::Y
    }

    pub fn translate_local(&mut self, translation: Vec3) {
        self.local_position += self.local_rotation * translation;
    }

    pub fn look_at(&mut self, target: Vec3, world_up: Vec3) {
        let forward = (target - self.local_position).normalize_or_zero();
        if forward == Vec3::ZERO {
            return;
        }

        let rotation_matrix = Mat4::look_at_rh(Vec3::ZERO, forward, world_up).inverse();
        self.local_rotation = Quat::from_mat4(&rotation_matrix);
    }

    pub fn local_to_world_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            self.local_scale,
            self.local_rotation,
            self.local_position,
        )
    }
}

impl Default for LocalTransform {
    fn default() -> Self {
        LocalTransform::IDENTITY
    }
}

#[derive(Component, Clone, Copy)]
pub struct GlobalTransform(pub Mat4);

impl Default for GlobalTransform {
    fn default() -> Self {
        Self(Mat4::from_scale_rotation_translation(
            Vec3::ONE,
            Quat::IDENTITY,
            Vec3::ZERO,
        ))
    }
}
