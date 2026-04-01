use bevy_ecs::component::Component;
use connect_shared::*;

#[derive(Default, Clone, Copy)]
pub struct ClippingPlanes {
    pub near: f32,
    pub far: f32,
}

#[derive(Default, Component)]
#[require(LocalTransform)]
pub struct Camera {
    pub fov: f32,
    pub clipping_planes: ClippingPlanes,
}

impl Camera {
    pub fn new(fov: f32, near: f32, far: f32) -> Self {
        /*         let camera_rig = CameraRig::builder()
                   .with(Position::new(Vec3::new(85.45, -5.0, 2.52).to_array()))
                   .with(YawPitch::default())
                   .with(Smooth::new_position_rotation(1.0, 1.0))
                   .build();
        */
        Self {
            fov,
            clipping_planes: ClippingPlanes { near, far },
        }
    }

    /*     pub fn get_position(&self) -> Vec3 {
        let position = self.camera_rig.driver::<Position>().position;

        Vec3::new(position.x, position.y, position.z)
    }

    pub fn get_rotation(&self) -> Quat {
        let yaw_pitch = self.camera_rig.driver::<YawPitch>();

        Quat::from_euler(
            EulerRot::YXZ,
            yaw_pitch.yaw_degrees.to_radians(),
            yaw_pitch.pitch_degrees.to_radians(),
            Default::default(),
        )
     }*/

    /*     pub fn process_mouse(&mut self, delta_x: f32, delta_y: f32) {
        self.mouse_delta = Vec2::new(delta_x, delta_y).normalize();

        self.camera_rig.driver_mut::<YawPitch>().rotate_yaw_pitch(
            -self.rotation_speed * self.mouse_delta.x,
            -self.rotation_speed * self.mouse_delta.y,
        );
    } */

    /*     pub fn update(&mut self) {
       let current_rotation = self.get_rotation();

       let camera_position = self.camera_rig.driver_mut::<Position>();
       let mut original_position = camera_position.position;
       let mut original_position_vec = Vec3::new(
           original_position.x,
           original_position.y,
           original_position.z,
       );

       let forward = current_rotation * -Vec3::Z;
       let right = current_rotation * Vec3::X;
       let up = Vec3::Y;

       let move_speed = self.move_speed;
       self.keyboard_state.iter().for_each(|(key_code, state)| {
           let is_pressed = state.is_pressed();

           if is_pressed {
               match key_code {
                   KeyCode::KeyW => {
                       original_position_vec += move_speed * forward;
                   }
                   KeyCode::KeyA => {
                       original_position_vec -= move_speed * right;
                   }
                   KeyCode::KeyS => {
                       original_position_vec -= move_speed * forward;
                   }
                   KeyCode::KeyD => {
                       original_position_vec += move_speed * right;
                   }
                   KeyCode::KeyE => {
                       original_position_vec += move_speed * up;
                   }
                   KeyCode::KeyQ => {
                       original_position_vec -= move_speed * up;
                   }
                   _ => {}
               }
           }
       });

       original_position.x = original_position_vec.x;
       original_position.y = original_position_vec.y;
       original_position.z = original_position_vec.z;

       camera_position.position = original_position;

       self.camera_rig.update(delta_time);
    }*/
}
