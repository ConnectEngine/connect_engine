// TODO: MOVE TO THE PROPER FOLDER!

use std::{
    hash::{Hash, Hasher},
    path::Path,
};

use ahash::AHasher;
use bevy_ecs::resource::Resource;
use connect_shared::AudioKey;
use kira::{
    AudioManager, AudioManagerSettings, DefaultBackend,
    sound::{
        PlaybackState,
        static_sound::{StaticSoundData, StaticSoundHandle},
    },
};
use slotmap::SlotMap;

#[derive(Default, Clone, Copy)]
pub struct AudioReference {
    pub key: AudioKey,
    hash: u64,
}

impl AudioReference {
    pub fn get_hash(&self) -> u64 {
        self.hash
    }
}

struct AudioContainer {
    pub static_sound_data: StaticSoundData,
    pub hash: u64,
}

struct AudioHandle {
    pub static_sound_handles: StaticSoundHandle,
}

pub struct AudioHandleReference {
    pub key: AudioKey,
}

#[derive(Resource)]
pub struct Audio {
    audio_manager: AudioManager,
    audios: SlotMap<AudioKey, AudioContainer>,
    active_audio_handlers: SlotMap<AudioKey, AudioHandle>,
    hasher: ahash::AHasher,
}

impl Audio {
    pub(crate) fn new() -> Self {
        let audio_manager =
            AudioManager::<DefaultBackend>::new(AudioManagerSettings::default()).unwrap();

        Self {
            audio_manager,
            audios: SlotMap::with_capacity_and_key(u8::MAX as _),
            active_audio_handlers: SlotMap::with_capacity_and_key(u8::MAX as _),
            hasher: AHasher::default(),
        }
    }

    pub fn load_audio(&mut self, path: &Path) -> AudioReference {
        path.hash(&mut self.hasher);
        let path_hash = self.hasher.finish();

        let probably_found_already_loaded_audio = self
            .audios
            .iter()
            .find(|(_, audio_container)| audio_container.hash == path_hash);

        let audio_key = match probably_found_already_loaded_audio {
            Some((audio_key, _)) => audio_key,
            None => {
                let static_sound_data = StaticSoundData::from_file(path).unwrap();
                let audio_container = AudioContainer {
                    static_sound_data,
                    hash: path_hash,
                };

                self.audios.insert(audio_container)
            }
        };

        AudioReference {
            key: audio_key,
            hash: path_hash,
        }
    }

    pub(crate) fn check_audio_state(&mut self) {
        self.active_audio_handlers.retain(|_, audio_handle| {
            audio_handle.static_sound_handles.state() == PlaybackState::Stopped
        });
    }

    pub fn play_audio(
        &mut self,
        audio_reference: AudioReference,
        is_looped: bool,
    ) -> AudioHandleReference {
        let audio_container = unsafe { self.audios.get(audio_reference.key).unwrap_unchecked() };
        let mut static_sound_handle = self
            .audio_manager
            .play(audio_container.static_sound_data.clone())
            .unwrap();

        if is_looped {
            static_sound_handle.set_loop_region(..);
        }

        let audio_handle = AudioHandle {
            static_sound_handles: static_sound_handle,
        };

        let audio_handle_reference = self.active_audio_handlers.insert(audio_handle);

        AudioHandleReference {
            key: audio_handle_reference,
        }
    }
}
