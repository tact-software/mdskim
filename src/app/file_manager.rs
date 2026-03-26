use std::path::{Path, PathBuf};

/// Manages multi-file navigation and current file tracking.
pub(crate) struct FileManager {
    pub(crate) file_path: Option<PathBuf>,
    pub(crate) file_list: Vec<PathBuf>,
    pub(crate) file_index: usize,
}

impl FileManager {
    pub fn new(file_path: Option<PathBuf>) -> Self {
        Self {
            file_path,
            file_list: Vec::new(),
            file_index: 0,
        }
    }

    /// Derive the file name from file_path.
    pub fn file_name(&self) -> Option<&Path> {
        self.file_path
            .as_deref()
            .and_then(|p| p.file_name().map(Path::new))
    }
}
