use std::collections::HashMap;
use std::path::PathBuf;

/// Cache for pre-rendered Mermaid, math, and image assets.
#[derive(Default)]
pub(crate) struct RenderCache {
    pub(crate) mermaid_images: HashMap<usize, PathBuf>,
    pub(crate) math_images: HashMap<usize, PathBuf>,
    pub(crate) image_paths: HashMap<usize, PathBuf>,
    pub(crate) mermaid_errors: HashMap<usize, String>,
    pub(crate) math_errors: HashMap<usize, String>,
    pub(crate) generation: u64,
}

impl RenderCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.mermaid_images.clear();
        self.math_images.clear();
        self.image_paths.clear();
        self.mermaid_errors.clear();
        self.math_errors.clear();
        self.generation += 1;
    }
}
