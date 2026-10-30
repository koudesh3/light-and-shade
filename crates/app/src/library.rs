//! Where paintings live on disk.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// `LAS_PROJECTS`, else the repo's `projects/` when running from a checkout, else a
/// folder in Documents that starts with a copy of the bundled paintings.
pub fn root() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os("LAS_PROJECTS") {
        return Ok(dir.into());
    }
    let exe = std::env::current_exe().ok();
    let starts = [std::env::current_dir().ok(), exe.clone()];
    let checkout = starts
        .iter()
        .flatten()
        .flat_map(|start| start.ancestors())
        .map(|dir| dir.join("projects"))
        .find(|dir| dir.join("pothos/project.json").is_file());
    if let Some(dir) = checkout {
        return Ok(dir);
    }
    let dir = dirs::document_dir()
        .context("couldn't find your Documents folder")?
        .join("Light and Shade");
    if !dir.exists() {
        std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        let bundled = exe
            .as_deref()
            .and_then(Path::parent)
            .map(|d| d.join("../Resources/projects"));
        if let Some(bundled) = bundled.filter(|b| b.is_dir()) {
            copy_tree(&bundled, &dir)?;
        }
    }
    Ok(dir)
}

fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}
