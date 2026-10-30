//! Finds painting projects on disk.

use las_core::Project;
use std::path::{Path, PathBuf};

/// `LAS_PROJECTS`, else the nearest `projects/` folder above the working directory or the
/// binary. A macOS bundle keeps its own copy in `Contents/Resources`.
pub fn root() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("LAS_PROJECTS") {
        return Some(dir.into());
    }
    let starts = [std::env::current_dir().ok(), std::env::current_exe().ok()];
    starts
        .iter()
        .flatten()
        .flat_map(|start| start.ancestors())
        .flat_map(|dir| [dir.join("projects"), dir.join("Resources/projects")])
        .find(|dir| dir.is_dir())
}

/// Readable project folders directly inside `root`, with their names, sorted by folder.
pub fn scan(root: &Path) -> Vec<(PathBuf, String)> {
    let mut found: Vec<(PathBuf, String)> = std::fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter_map(|path| Project::load(&path).ok().map(|p| (path, p.name)))
        .collect();
    found.sort();
    found
}
