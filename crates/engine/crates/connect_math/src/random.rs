use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use bevy_ecs::component::Component;
use bevy_ecs::resource::Resource;
use general::Vec2;
use rand::RngExt;
use rand::SeedableRng;
use rand::distr::StandardUniform;
use rand::distr::uniform::*;
use rand_xoshiro::Xoshiro256PlusPlus;

#[derive(Resource, Component)]
pub struct Random {
    xoshiro256_plus_plus: Xoshiro256PlusPlus,
}

impl Random {
    #[inline(always)]
    pub fn new() -> Self {
        Default::default()
    }

    #[inline(always)]
    pub fn set_seed(&mut self, state: u64) {
        self.xoshiro256_plus_plus = Xoshiro256PlusPlus::seed_from_u64(state);
    }

    #[inline(always)]
    pub fn range<T: SampleUniform, R: SampleRange<T>>(&mut self, range: R) -> T {
        self.xoshiro256_plus_plus.random_range(range)
    }

    #[inline(always)]
    pub fn random<T>(&mut self) -> T
    where
        StandardUniform: rand::distr::Distribution<T>,
    {
        self.xoshiro256_plus_plus.random()
    }

    #[inline(always)]
    pub fn random_bool(&mut self, propability: f32) -> bool {
        self.xoshiro256_plus_plus.random_bool(propability as _)
    }

    #[inline(always)]
    pub fn inside_unit_circle(&mut self) -> Vec2 {
        let result: Vec2;

        loop {
            let x = self.range(-1.0..=1.0);
            let y = self.range(-1.0..=1.0);

            if x * x + y * y <= 1.0 {
                result = Vec2::new(x, y);

                break;
            }
        }

        result
    }
}

impl Default for Random {
    #[inline(always)]
    fn default() -> Self {
        Self {
            xoshiro256_plus_plus: get_new_xoshiro256_plus_plus_instance(),
        }
    }
}

#[derive(Resource)]
pub struct ThreadedRandom {
    master_xoshiro256_plus_plus: Xoshiro256PlusPlus,
}

impl ThreadedRandom {
    #[inline(always)]
    pub fn new() -> Self {
        Default::default()
    }

    #[inline(always)]
    pub fn spawn_random(&mut self) -> Random {
        let child_xoshiro256_plus_plus = self.master_xoshiro256_plus_plus.clone();
        self.master_xoshiro256_plus_plus.jump();

        Random {
            xoshiro256_plus_plus: child_xoshiro256_plus_plus,
        }
    }
}

impl Default for ThreadedRandom {
    #[inline(always)]
    fn default() -> Self {
        Self {
            master_xoshiro256_plus_plus: get_new_xoshiro256_plus_plus_instance(),
        }
    }
}

#[inline(always)]
fn get_new_xoshiro256_plus_plus_instance() -> Xoshiro256PlusPlus {
    let state = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;

    Xoshiro256PlusPlus::seed_from_u64(state)
}
