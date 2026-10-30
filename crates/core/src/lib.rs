//! Project format and pure scene maths. No GPU and no UI in here.

mod bake;

use anyhow::{Context, Result};
use image::imageops::{self, FilterType};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub use bake::{THUMBNAIL, coverage, painting_name, slug};

/// Below this many pixels on the longest side, an image looks soft at full canvas size.
const LOW_RESOLUTION: u32 = 1600;

/// A piece traced out of the source image, as a closed polygon in source pixels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cut {
    pub name: String,
    pub points: Vec<[f32; 2]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layer {
    pub name: String,
    pub z: u32,
    pub x: f32,
    pub y: f32,
    pub w: u32,
    pub h: u32,
    pub color: PathBuf,
    pub normal: PathBuf,
}

/// One painting. Layers are stored back to front, in painting pixels. Cuts and the source
/// image are the editable originals; layers are baked from them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub source: Option<PathBuf>,
    #[serde(default)]
    pub cuts: Vec<Cut>,
    #[serde(default)]
    pub layers: Vec<Layer>,
    #[serde(skip)]
    pub dir: PathBuf,
}

impl Project {
    pub fn load(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref();
        let path = dir.join("project.json");
        let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let mut project: Project =
            serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        project.dir = dir.to_path_buf();
        project.layers.sort_by_key(|l| l.z);
        Ok(project)
    }

    pub fn save(&self) -> Result<()> {
        let path = self.dir.join("project.json");
        let temp = self.dir.join("project.json.tmp");
        std::fs::write(&temp, serde_json::to_string_pretty(self)? + "\n")
            .with_context(|| format!("writing {}", temp.display()))?;
        std::fs::rename(&temp, &path).with_context(|| format!("replacing {}", path.display()))
    }

    /// Whether the source is too small to stay sharp on a large canvas.
    pub fn is_low_resolution(&self) -> bool {
        self.is_cuttable() && self.width.max(self.height) < LOW_RESOLUTION
    }

    /// Whether this painting has a source image to cut from.
    pub fn is_cuttable(&self) -> bool {
        self.source.is_some()
    }
}

/// One mip level of RGBA8 pixels.
pub struct Mip {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// What a layer's image holds, which decides how it is prepared for filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapKind {
    Color,
    Normal,
}

/// Decode an image and build its mip chain. Filtering must not pull the black behind
/// a leaf into its edges, so colour is premultiplied and normals are grown outward.
pub fn load_mips(path: &Path, kind: MapKind) -> Result<Vec<Mip>> {
    let mut img = image::open(path)
        .with_context(|| format!("decoding {}", path.display()))?
        .into_rgba8();
    match kind {
        MapKind::Color => {
            for p in img.pixels_mut() {
                let a = p[3] as u32;
                for c in &mut p.0[..3] {
                    *c = ((*c as u32 * a + 127) / 255) as u8;
                }
            }
        }
        MapKind::Normal => grow_normals(&mut img),
    }
    let mut mips = Vec::new();
    loop {
        let (width, height) = img.dimensions();
        let next = (width > 1 || height > 1).then(|| {
            imageops::resize(
                &img,
                (width / 2).max(1),
                (height / 2).max(1),
                FilterType::Triangle,
            )
        });
        mips.push(Mip {
            width,
            height,
            rgba: img.into_raw(),
        });
        match next {
            Some(n) => img = n,
            None => return Ok(mips),
        }
    }
}

/// Outside a leaf the maps are black, and downscaling smeared that into the edge pixels.
/// Neither decodes to a unit normal facing the viewer, so treat them as unknown: spread the
/// nearest real normals a few pixels over them, and call anything further away flat.
fn grow_normals(img: &mut image::RgbaImage) {
    const REACH: usize = 12;
    let unknown = |p: &image::Rgba<u8>| {
        let n = [0, 1, 2].map(|c| p[c] as f32 / 127.5 - 1.0);
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        n[2] < 0.0 || !(0.9..=1.1).contains(&len)
    };
    let (w, h) = (img.width() as i32, img.height() as i32);
    for _ in 0..REACH {
        let snapshot = img.clone();
        for y in 0..h {
            for x in 0..w {
                if !unknown(snapshot.get_pixel(x as u32, y as u32)) {
                    continue;
                }
                let (mut sum, mut n) = ([0u32; 3], 0);
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= w || ny >= h {
                        continue;
                    }
                    let p = snapshot.get_pixel(nx as u32, ny as u32);
                    if !unknown(p) {
                        (0..3).for_each(|c| sum[c] += p[c] as u32);
                        n += 1;
                    }
                }
                if n > 0 {
                    let p = img.get_pixel_mut(x as u32, y as u32);
                    (0..3).for_each(|c| p[c] = (sum[c] / n) as u8);
                }
            }
        }
    }
    for p in img.pixels_mut() {
        if unknown(p) {
            *p = image::Rgba([128, 128, 255, 255]);
        }
    }
}

/// The light, as a point above the painting: x right, y up, `height` toward the viewer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Light {
    pub x: f32,
    pub y: f32,
    pub height: f32,
}

impl Light {
    pub fn direction(self) -> [f32; 3] {
        let l = (self.x * self.x + self.y * self.y + self.height * self.height)
            .sqrt()
            .max(1e-6);
        [self.x / l, self.y / l, self.height / l]
    }
}

/// How the painting sits in a viewport. `pan` is in target pixels, `zoom` is relative to fit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    pub zoom: f32,
    pub pan: [f32; 2],
}

impl Default for View {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: [0.0, 0.0],
        }
    }
}

impl View {
    const PAD: f32 = 0.92;

    /// Target pixels per painting pixel.
    pub fn scale(&self, target: [f32; 2], painting: [f32; 2]) -> f32 {
        (target[0] / painting[0]).min(target[1] / painting[1]) * Self::PAD * self.zoom
    }

    /// Painting origin in target pixels.
    pub fn origin(&self, target: [f32; 2], painting: [f32; 2]) -> [f32; 2] {
        let s = self.scale(target, painting);
        [
            (target[0] - painting[0] * s) / 2.0 + self.pan[0],
            (target[1] - painting[1] * s) / 2.0 + self.pan[1],
        ]
    }

    /// Painting pixels to clip space, as `[sx, sy, tx, ty]`.
    pub fn transform(&self, target: [f32; 2], painting: [f32; 2]) -> [f32; 4] {
        let s = self.scale(target, painting);
        let o = self.origin(target, painting);
        [
            2.0 * s / target[0],
            -2.0 * s / target[1],
            2.0 * o[0] / target[0] - 1.0,
            1.0 - 2.0 * o[1] / target[1],
        ]
    }

    /// Zoom by `factor`, keeping the painting point under `at` (target pixels) still.
    pub fn zoom_at(&mut self, factor: f32, at: [f32; 2], target: [f32; 2], painting: [f32; 2]) {
        let (s0, o0) = (self.scale(target, painting), self.origin(target, painting));
        let p = [(at[0] - o0[0]) / s0, (at[1] - o0[1]) / s0];
        self.zoom = (self.zoom * factor).clamp(0.1, 40.0);
        let (s1, o1) = (self.scale(target, painting), self.origin(target, painting));
        self.pan[0] += at[0] - (o1[0] + p[0] * s1);
        self.pan[1] += at[1] - (o1[1] + p[1] * s1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projects_without_cuts_still_load() {
        let p = Project::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../projects/pothos")).unwrap();
        assert!(p.cuts.is_empty() && !p.is_cuttable());
    }

    #[test]
    fn small_sources_are_flagged() {
        let mut p = Project::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../projects/pothos")).unwrap();
        p.source = Some("source/original.png".into());
        (p.width, p.height) = (768, 1024);
        assert!(p.is_low_resolution());
        (p.width, p.height) = (3024, 4032);
        assert!(!p.is_low_resolution());
    }

    #[test]
    fn loads_pothos_back_to_front() {
        let p = Project::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../projects/pothos")).unwrap();
        assert_eq!((p.width, p.height, p.layers.len()), (2000, 1801, 17));
        assert!(p.layers.windows(2).all(|w| w[0].z < w[1].z));
    }

    #[test]
    fn mips_reach_one_pixel_and_premultiply() {
        let p = Project::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../projects/pothos")).unwrap();
        let mips = load_mips(&p.dir.join(&p.layers[1].color), MapKind::Color).unwrap();
        let last = mips.last().unwrap();
        assert_eq!((last.width, last.height), (1, 1));
        assert!(
            mips[0]
                .rgba
                .chunks(4)
                .all(|c| c[0] <= c[3] && c[1] <= c[3] && c[2] <= c[3])
        );
    }

    #[test]
    fn normals_have_no_black_left_to_filter_in() {
        let p = Project::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../projects/pothos")).unwrap();
        for layer in &p.layers {
            let mips = load_mips(&p.dir.join(&layer.normal), MapKind::Normal).unwrap();
            assert!(mips[0].rgba.chunks(4).all(|c| c[2] > 100), "{}", layer.name);
        }
    }

    #[test]
    fn light_direction_is_unit() {
        let d = Light {
            x: -0.55,
            y: 0.5,
            height: 0.55,
        }
        .direction();
        assert!((d.iter().map(|c| c * c).sum::<f32>() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn zoom_keeps_the_point_under_the_cursor() {
        let (t, p) = ([1000.0, 800.0], [2000.0, 1801.0]);
        let mut v = View::default();
        let at = [320.0, 250.0];
        let (s, o) = (v.scale(t, p), v.origin(t, p));
        let before = [(at[0] - o[0]) / s, (at[1] - o[1]) / s];
        v.zoom_at(2.5, at, t, p);
        let (s, o) = (v.scale(t, p), v.origin(t, p));
        assert!(((at[0] - o[0]) / s - before[0]).abs() < 0.01);
        assert!(((at[1] - o[1]) / s - before[1]).abs() < 0.01);
    }
}
