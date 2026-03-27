use std::path::{Path, PathBuf};

use bevy_ecs::resource::Resource;

pub struct EditorApplication {
    executable_path_buf: PathBuf,
    assets_folder_path_buf: PathBuf,
    artifacts_folder_path_buf: PathBuf,
}

impl EditorApplication {
    pub fn new() -> Self {
        let mut executable_path_buf = std::env::current_exe().unwrap();

        executable_path_buf.pop();
        executable_path_buf.pop();
        executable_path_buf.pop();
        let assets_folder_path_buf = executable_path_buf.join("assets");
        let artifacts_folder_path_buf = executable_path_buf.join("artifacts");

        Self {
            executable_path_buf,
            assets_folder_path_buf,
            artifacts_folder_path_buf,
        }
    }

    pub fn get_executable_path(&self) -> &Path {
        &self.executable_path_buf
    }

    pub fn get_assets_folder_path(&self) -> &Path {
        &self.assets_folder_path_buf
    }

    pub fn get_artifacts_folder_path(&self) -> &Path {
        &self.artifacts_folder_path_buf
    }
}

#[derive(Resource)]
pub struct Information {
    editor_application: EditorApplication,
}

impl Information {
    pub fn new() -> Self {
        Self {
            editor_application: EditorApplication::new(),
        }
    }

    pub fn get_editor_application(&self) -> &EditorApplication {
        &self.editor_application
    }
}
