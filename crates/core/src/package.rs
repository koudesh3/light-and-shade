//! `.lightshade` files: a painting's folder as a single zip, for sharing and backups.
//! Packages can come from anywhere, so importing treats them as untrusted.

use crate::Project;
use anyhow::{Context, Result, bail};
use std::fs::File;
use std::io::{self, BufWriter, Read, Write};
use std::path::{Component, Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

pub const PACKAGE_EXTENSION: &str = "lightshade";
/// More than any real painting needs, to stop a tiny file unpacking into a huge one.
const MAX_UNPACKED: u64 = 8 * 1024 * 1024 * 1024;
/// Covers, past and present. The home page redraws them, so they don't need to travel.
const LEFT_OUT: [&str; 3] = ["cover-split.png", "cover.png", "thumbnail.png"];

fn files_in(dir: &Path, found: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            files_in(&entry.path(), found)?;
        } else if kind.is_file() {
            found.push(entry.path());
        }
    }
    Ok(())
}

/// Stays inside the project: relative, with no `..`.
fn is_inside(path: &Path) -> bool {
    path.components()
        .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

impl Project {
    /// Write this painting to a single file.
    pub fn export(&self, path: &Path) -> Result<()> {
        let mut files = Vec::new();
        files_in(&self.dir, &mut files)?;
        files.sort();
        // Write beside the target first, so a failure can't leave half a file there.
        let temp = path.with_extension("lightshade.tmp");
        let mut zip = ZipWriter::new(BufWriter::new(
            File::create(&temp).with_context(|| format!("creating {}", temp.display()))?,
        ));
        for file in &files {
            let relative = file.strip_prefix(&self.dir)?;
            if LEFT_OUT.iter().any(|name| relative == Path::new(name)) {
                continue;
            }
            let name: Vec<String> = relative
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            // Images are already compressed, so only the project file is worth squeezing.
            let compressed = relative.extension().is_some_and(|e| e == "json");
            let method = if compressed {
                CompressionMethod::Deflated
            } else {
                CompressionMethod::Stored
            };
            let options = SimpleFileOptions::default()
                .compression_method(method)
                .large_file(true);
            zip.start_file(name.join("/"), options)?;
            io::copy(&mut File::open(file)?, &mut zip)?;
        }
        zip.finish()?.flush()?;
        std::fs::rename(&temp, path).with_context(|| format!("writing {}", path.display()))
    }

    /// Unpack a `.lightshade` file into a new painting under `root`.
    pub fn import(root: &Path, package: &Path) -> Result<Self> {
        let file = File::open(package).with_context(|| format!("opening {}", package.display()))?;
        let mut archive = ZipArchive::new(file).context("this isn't a Light and Shade file")?;
        let unpacking = root.join(format!(".unpacking-{}", std::process::id()));
        std::fs::create_dir_all(&unpacking)?;
        let result = unpack(&mut archive, &unpacking).and_then(|()| finish_import(root, &unpacking));
        if result.is_err() {
            let _ = std::fs::remove_dir_all(&unpacking);
        }
        result
    }
}

fn unpack(archive: &mut ZipArchive<File>, into: &Path) -> Result<()> {
    let mut left = MAX_UNPACKED;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        if entry.is_symlink() {
            bail!("this file contains a link, which a painting never should");
        }
        let Some(name) = entry.enclosed_name() else {
            bail!("this file tries to write outside its own folder");
        };
        let target = into.join(name);
        if entry.is_dir() {
            std::fs::create_dir_all(&target)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = BufWriter::new(File::create(&target)?);
        // One past the budget, so a file that lies about its size is still caught.
        let copied = io::copy(&mut (&mut entry).take(left + 1), &mut out)?;
        if copied > left {
            bail!("this file is too large to be a painting");
        }
        left -= copied;
    }
    Ok(())
}

fn finish_import(root: &Path, unpacked: &Path) -> Result<Project> {
    let project = Project::load(unpacked).context("this isn't a Light and Shade file")?;
    let mut paths: Vec<&Path> = project.source.iter().map(PathBuf::as_path).collect();
    for layer in &project.layers {
        paths.push(&layer.color);
        paths.push(&layer.normal);
    }
    if !paths.iter().all(|p| is_inside(p) && unpacked.join(p).is_file()) {
        bail!("this file refers to images that aren't inside it");
    }
    let dir = crate::bake::unused_folder(root, &crate::slug(&project.name));
    std::fs::rename(unpacked, &dir).with_context(|| format!("creating {}", dir.display()))?;
    Project::load(&dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Cut;
    use image::{Rgba, RgbaImage};

    fn workspace(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("las-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn project(root: &Path) -> Project {
        let image = root.join("fern.png");
        RgbaImage::from_pixel(40, 40, Rgba([20, 120, 40, 255]))
            .save(&image)
            .unwrap();
        let mut project = Project::create(root, &image).unwrap();
        project.cuts = vec![Cut::new(
            "Leaf".into(),
            vec![[5.0, 5.0], [30.0, 5.0], [30.0, 30.0]],
        )];
        project.bake().unwrap();
        project
    }

    fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let mut zip = ZipWriter::new(File::create(path).unwrap());
        for (name, bytes) in entries {
            zip.start_file(*name, SimpleFileOptions::default()).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn a_painting_survives_export_and_import() {
        let root = workspace("roundtrip");
        let original = project(&root);
        let package = root.join("Fern.lightshade");
        original.export(&package).unwrap();

        let imported = Project::import(&root, &package).unwrap();
        assert_ne!(imported.dir, original.dir, "an import never overwrites");
        assert_eq!(imported.name, original.name);
        assert_eq!(imported.cuts, original.cuts);
        assert!(
            !imported.dir.join(crate::THUMBNAIL).exists(),
            "covers are redrawn"
        );
        let a = std::fs::read(original.dir.join(&original.layers[0].color)).unwrap();
        let b = std::fs::read(imported.dir.join(&imported.layers[0].color)).unwrap();
        assert_eq!(a, b);
        let leftovers = std::fs::read_dir(&root).unwrap().flatten();
        assert!(
            leftovers
                .into_iter()
                .all(|e| !e.file_name().to_string_lossy().starts_with('.'))
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn packages_cannot_write_outside_their_folder() {
        let root = workspace("slip");
        let library = root.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let package = root.join("bad.lightshade");
        write_zip(&package, &[("../escaped.txt", b"nope"), ("project.json", b"{}")]);
        assert!(Project::import(&library, &package).is_err());
        assert!(!root.join("escaped.txt").exists());
        assert_eq!(
            std::fs::read_dir(&library).unwrap().count(),
            0,
            "nothing is left behind"
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn packages_cannot_point_at_files_outside_themselves() {
        let root = workspace("refs");
        let package = root.join("bad.lightshade");
        let text = br#"{ "name": "Bad", "width": 4, "height": 4, "source": "../../secret.png" }"#;
        write_zip(&package, &[("project.json", text)]);
        assert!(Project::import(&root, &package).is_err());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn other_files_are_refused() {
        let root = workspace("junk");
        let package = root.join("notes.lightshade");
        std::fs::write(&package, "not a zip").unwrap();
        assert!(Project::import(&root, &package).is_err());
        let empty = root.join("empty.lightshade");
        write_zip(&empty, &[("readme.txt", b"hello")]);
        assert!(Project::import(&root, &empty).is_err());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
