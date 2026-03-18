use asset_importer::{Importer, Scene, postprocess::PostProcessSteps};

pub struct ModelLoader {
    importer: Importer,
}

impl ModelLoader {
    pub fn new() -> Self {
        Self {
            importer: Importer::new(),
        }
    }

    pub fn load_model(&self, path: &str) -> Scene {
        self.importer
            .read_file(path)
            .with_post_process(PostProcessSteps::MAX_QUALITY | PostProcessSteps::FLIP_UVS)
            .import()
            .unwrap()
    }
}
