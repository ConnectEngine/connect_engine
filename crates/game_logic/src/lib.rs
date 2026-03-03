use core::panic;
use std::path::PathBuf;

use bevy_ecs::{
    component::Component,
    entity::{Entity, EntityCloner},
    entity_disabling::Disabled,
    hierarchy::Children,
    query::{Has, With},
    relationship::RelationshipTarget,
    system::{Command, Commands, Local, Query, Res, ResMut},
    world::World,
};
use engine::{
    GamePlugin,
    engine::{
        AudioReference, Camera, ClippingPlanes, Input, LoadModelEvent, LocalTransform, Mesh,
        Physics, RigidBody, Time, Transform,
    },
};
use engine::{engine::Audio, math::*};
use winit::keyboard::KeyCode;

#[unsafe(no_mangle)]
pub extern "Rust" fn get_game() -> Box<dyn GamePlugin> {
    Box::new(Game)
}

struct Game;

impl GamePlugin for Game {
    fn add_systems_init(&self, schedule: &mut bevy_ecs::schedule::Schedule) {
        schedule.add_systems((spawn_planet, play_audio, spawn_player));
    }

    fn add_systems_update(&self, schedule: &mut bevy_ecs::schedule::Schedule) {
        schedule.add_systems((
            move_player,
            fire_from_gun,
            spawn_asteroids,
            create_rigidbody_for_planet,
            rotate_asteroids,
            rotate_player,
            jump_player,
        ));
    }
}

#[derive(Component)]
pub struct FireAudioHandle {
    pub audio_reference: AudioReference,
}

#[derive(Component)]
#[require(Camera)]
pub struct PlayerStats {
    pub move_speed: f32,
    pub run_speed: f32,
    pub rotation_speed: f32,
}

#[derive(Default, Component)]
pub struct PlayerJump {
    pub jump_duration: f32,
    pub jump_height: f32,
    pub current_duration: f32,
    pub initial_y_height: f32,
    pub is_jumping: bool,
    pub is_falling: bool,
}

#[derive(Component)]
#[require(LocalTransform)]
pub struct PlanetTag;

#[derive(Component)]
#[require(LocalTransform)]
pub struct AsteroidInstance {
    rotation_axis: AsteroidRotationAxis,
}

pub trait Prefab {
    fn instantiate(&self, commands: Commands) -> Entity;
}

#[derive(Component)]
#[require(LocalTransform)]
pub struct AsteroidPrefab;

#[derive(Clone, Copy, PartialEq, Eq)]
enum AsteroidRotationAxis {
    X,
    Y,
    Z,
}

struct CloneHierarchyCommand {
    pub source: Entity,
    pub position: Vec3,
    pub scale: Vec3,
    pub rotation: Vec3,
    pub asteroid_rotation_axis: AsteroidRotationAxis,
}

impl Command for CloneHierarchyCommand {
    fn apply(self, world: &mut World) {
        let mut entity_cloner_builder = EntityCloner::build_opt_out(world);
        entity_cloner_builder.linked_cloning(true);
        let mut entity_cloner = entity_cloner_builder.finish();

        let entity = entity_cloner.spawn_clone(world, self.source);
        let mut entity = world.entity_mut(entity);
        let mut entity_transform = entity.get_mut::<LocalTransform>().unwrap();
        entity_transform.local_position = self.position;
        entity_transform.local_scale = self.scale;
        entity_transform.set_local_euler_angles(self.rotation);

        entity.insert(AsteroidInstance {
            rotation_axis: self.asteroid_rotation_axis,
        });
        entity.remove_recursive::<Children, Disabled>();
        entity.remove::<AsteroidPrefab>();
    }
}

fn play_audio(mut commands: Commands, mut audio: ResMut<Audio>) {
    // TODO: Deduplicate and simplify.
    let mut exe_path = std::env::current_exe().unwrap();

    exe_path.pop();
    exe_path.pop();
    exe_path.pop();

    let _sound = audio.load_audio(&PathBuf::from(std::format!(
        "{}/assets/audio/sound.mp3",
        exe_path.as_os_str().display()
    )));
    //audio.play_audio(sound, true);

    let fire_sound = audio.load_audio(&PathBuf::from(std::format!(
        "{}/assets/audio/fire.mp3",
        exe_path.as_os_str().display()
    )));

    let mut fire_audio_handle_entity = commands.spawn_empty();
    fire_audio_handle_entity.insert(FireAudioHandle {
        audio_reference: fire_sound,
    });
}

fn fire_from_gun(
    input: Res<Input>,
    fire_audio_handle_query: Query<&FireAudioHandle>,
    mut audio: ResMut<Audio>,
) {
    if input.just_pressed(KeyCode::ArrowLeft)
        && let Ok(fire_audio_handle) = fire_audio_handle_query.single()
    {
        audio.play_audio(fire_audio_handle.audio_reference, false);
    }
}

fn spawn_planet(mut commands: Commands) {
    // TODO: Deduplicate and simplify.
    let mut exe_path = std::env::current_exe().unwrap();

    exe_path.pop();
    exe_path.pop();
    exe_path.pop();

    let planet_scale = 20.0;
    let mut planet_transform = LocalTransform::IDENTITY;
    planet_transform.local_scale *= planet_scale;

    let planet_entity = commands.spawn((PlanetTag, planet_transform));
    let planet_entity_id = planet_entity.id();

    commands.trigger(LoadModelEvent {
        path: PathBuf::from(std::format!(
            "{}/assets/planet.glb",
            exe_path.as_os_str().display()
        )),
        parent_entity: Some(planet_entity_id),
    });

    let asteroid = 1.0;
    let mut asteroid_transform = LocalTransform::IDENTITY;
    asteroid_transform.local_scale *= asteroid;

    let asteroid_entity = commands.spawn((AsteroidPrefab, Disabled, asteroid_transform));
    let asteroid_entity_id = asteroid_entity.id();

    commands.trigger(LoadModelEvent {
        path: PathBuf::from(std::format!(
            "{}/assets/asteroid.glb",
            exe_path.as_os_str().display()
        )),
        parent_entity: Some(asteroid_entity_id),
    });
}

fn spawn_asteroids(
    mut commands: Commands,
    planet_query: Query<&LocalTransform, With<PlanetTag>>,
    asteroid_prefab_query: Query<(Entity, Option<&Children>, Has<Disabled>), With<AsteroidPrefab>>,
    mut random: ResMut<Random>,
    mut has_spawned: Local<bool>,
) {
    if !*has_spawned
        && let Ok((asteroid_prefab_entity, children, _)) = asteroid_prefab_query.single()
        && children.is_some()
        && !children.unwrap().is_empty()
    {
        *has_spawned = true;

        let mut inner_radius = 50.0;
        let mut outer_radius = inner_radius * 10.0;
        let belt_radius = outer_radius - inner_radius;
        let belt_thicness = 4.5;

        let planet_transform = planet_query.single().unwrap();
        let planet_radius = planet_transform.local_scale.x * 2.0;
        if inner_radius <= planet_radius {
            inner_radius = planet_radius + 1.0;
            outer_radius = inner_radius + belt_radius;
        }

        for _ in 0..5_000 {
            let random_direction = random.inside_unit_circle().normalize();
            let random_distance = random
                .range(inner_radius.powi(2)..outer_radius.powi(2))
                .sqrt();

            let mut position = vec3(random_direction.x, 0.0, random_direction.y) * random_distance;
            position.y = random.range(-belt_thicness..belt_thicness);
            let scale = random.range(0.25..1.0);

            let asteroid_rotation_axis = match random.range(0..3) {
                0 => AsteroidRotationAxis::X,
                1 => AsteroidRotationAxis::Y,
                2 => AsteroidRotationAxis::Z,
                _ => panic!("Only X, Y, Z axis supported"),
            };

            commands.queue(CloneHierarchyCommand {
                source: asteroid_prefab_entity,
                position: planet_transform.local_position + position,
                scale: vec3(scale, scale, scale),
                rotation: vec3(
                    random.range(-360.0..360.0),
                    random.range(-360.0..360.0),
                    random.range(-360.0..360.0),
                ),
                asteroid_rotation_axis,
            });
        }
    }
}

fn create_rigidbody_for_planet(
    planet_query: Query<(Entity, Transform), With<PlanetTag>>,
    children_query: Query<&Children>,
    mesh_query: Query<&Mesh>,
    mut physics: Physics,
    mut is_created_rigidbody: Local<bool>,
) {
    physics.create_box_collider(None, Vec3::new(100.0, 5.0, 100.0), vec3(0.0, -40.0, 0.0));

    if !*is_created_rigidbody && let Ok((planet_entity, transform)) = planet_query.single() {
        let mut stack: Vec<Entity> = Vec::new();

        if let Ok(children) = children_query.get(planet_entity) {
            stack.extend(children.into_iter().copied().rev());
        } else {
            println!("Parent entity has no children to search.");
            return;
        }

        while let Some(current_entity) = stack.pop() {
            if let Ok(&mesh_component) = mesh_query.get(current_entity) {
                *is_created_rigidbody = true;

                let mut rigid_body = physics.create_rigid_body(planet_entity, transform.position());
                physics.create_convex_mesh_collider_from_mesh(
                    planet_entity,
                    mesh_component,
                    rigid_body,
                );
                rigid_body.apply_impulse(Vec3::new(100000.0 * 2.0, 0.0, 0.0), &mut physics);

                return;
            }

            if let Ok(children) = children_query.get(current_entity) {
                stack.extend(children.into_iter().copied().rev());
            }
        }
    }
}

fn rotate_asteroids(
    time: Res<Time>,
    mut asteroids_query: Query<(&mut LocalTransform, &AsteroidInstance)>,
) {
    let asteroid_speed = 1.0;
    let delta_time = time.get_delta_time();

    asteroids_query
        .par_iter_mut()
        .for_each(|(mut asteroid_transform, asteroid_instance)| {
            let mut euler_angles = asteroid_transform.get_local_euler_angles();

            match asteroid_instance.rotation_axis {
                AsteroidRotationAxis::X => {
                    euler_angles.x += asteroid_speed * delta_time;
                }
                AsteroidRotationAxis::Y => {
                    euler_angles.y += asteroid_speed * delta_time;
                }
                AsteroidRotationAxis::Z => {
                    euler_angles.z += asteroid_speed * delta_time;
                }
            }

            asteroid_transform.set_local_euler_angles(euler_angles);
        });
}

fn spawn_player(mut commands: Commands) {
    let camera_component = Camera {
        fov: 75.0,
        clipping_planes: ClippingPlanes {
            near: 0.1,
            far: 1000.0,
        },
    };
    let player_stats_component = PlayerStats {
        move_speed: 5.0,
        run_speed: 15.0,
        rotation_speed: 5.0,
    };

    let player_jump = PlayerJump {
        jump_duration: 0.9,
        jump_height: 4.0,
        ..Default::default()
    };

    let mut player_entity = commands.spawn_empty();
    let mut player_transform = LocalTransform::IDENTITY;
    player_transform.local_position.z = 150.0;
    player_transform.local_position.y = -5.0;

    player_entity.insert((
        camera_component,
        player_stats_component,
        player_jump,
        player_transform,
    ));
}

fn move_player(
    mut player_query: Query<(&mut LocalTransform, &PlayerStats, &PlayerJump)>,
    time: Res<Time>,
    input: Res<Input>,
) {
    let delta_time = time.get_delta_time();

    let (mut transform, player_stats, player_jump) = player_query.single_mut().unwrap();

    let target_speed = if input.pressed(KeyCode::ShiftLeft) && !player_jump.is_jumping {
        player_stats.run_speed
    } else {
        player_stats.move_speed
    };

    let forward = transform.forward();
    let right = transform.right();
    if input.pressed(KeyCode::KeyW) {
        transform.local_position += forward * target_speed * delta_time;
    }

    if input.pressed(KeyCode::KeyS) {
        transform.local_position -= forward * target_speed * delta_time;
    }

    if input.pressed(KeyCode::KeyA) {
        transform.local_position -= right * target_speed * delta_time;
    }

    if input.pressed(KeyCode::KeyD) {
        transform.local_position += right * target_speed * delta_time;
    }
}

fn rotate_player(
    mut player_query: Query<(&mut LocalTransform, &PlayerStats)>,
    time: Res<Time>,
    input: Res<Input>,
) {
    let delta_time = time.get_delta_time();
    let mouse_axis = input.get_mouse_axis();

    let (mut transform, player_stats) = player_query.single_mut().unwrap();

    let mut angles = transform.get_local_euler_angles();

    angles.y -= player_stats.rotation_speed * mouse_axis.x * delta_time;

    angles.x += player_stats.rotation_speed * mouse_axis.y * delta_time;
    angles.x = angles.x.clamp(-90.0, 90.0);

    transform.set_local_euler_angles(angles);
}

fn jump_player(
    mut player_query: Query<(&mut LocalTransform, &mut PlayerJump)>,
    time: Res<Time>,
    input: Res<Input>,
) {
    let delta_time = time.get_delta_time();

    let (mut transform, mut player_jump) = player_query.single_mut().unwrap();

    if player_jump.is_jumping {
        player_jump.current_duration += delta_time;

        let in_start = if player_jump.is_falling {
            player_jump.jump_duration / 2.0
        } else {
            Default::default()
        };

        let in_end = if player_jump.is_falling {
            player_jump.jump_duration
        } else {
            player_jump.jump_duration / 2.0
        };

        let jump_height = player_jump.initial_y_height + player_jump.jump_height;
        let out_start = if player_jump.is_falling {
            jump_height
        } else {
            player_jump.initial_y_height
        };

        let out_end = if player_jump.is_falling {
            player_jump.initial_y_height
        } else {
            jump_height
        };

        let new_height = player_jump
            .current_duration
            .remap(in_start, in_end, out_start, out_end);
        transform.local_position.y = new_height;
        player_jump.is_falling = player_jump.current_duration > player_jump.jump_duration / 2.0;

        if player_jump.is_falling
            && f32::abs(transform.local_position.y - player_jump.initial_y_height) < 0.1
        {
            player_jump.is_jumping = false;
            player_jump.is_falling = false;
        }
    } else {
        if input.just_pressed(KeyCode::Space) {
            player_jump.is_jumping = true;
            player_jump.is_falling = false;
            player_jump.current_duration = Default::default();
            player_jump.initial_y_height = transform.local_position.y;
        }
    }
}
