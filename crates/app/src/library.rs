//! Finds painting projects on disk.

use std::path::{Path, PathBuf};

/// `LAS_PROJECTS`, else the nearest `projects/` folder above the working directory or the binary.
pub fn root() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("LAS_PROJECTS") {
        return Some(dir.into());
    }
    let starts = [std::env::current_dir().ok(), std::env::current_exe().ok()];
    starts
        .iter()
        .flatten()
        .flat_map(|start| start.ancestors())
        .map(|dir| dir.join("projects"))
        .find(|dir| dir.is_dir())
}

/// Project folders directly inside `root`, by name.
pub fn scan(root: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.join("project.json").is_file())
        .collect();
    found.sort();
    found
}
