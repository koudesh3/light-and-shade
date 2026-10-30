//! Turning a dropped image into a project, and a project's cuts into layers.

use crate::{Cut, Layer, Project};
use anyhow::{Context, Result, bail};
use image::codecs::png::{CompressionType, FilterType as PngFilter, PngEncoder};
use image::imageops::{self, FilterType};
use image::{DynamicImage, ImageDecoder, ImageReader, RgbaImage};
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

const SOURCE: &str = "source/original.png";
const FLAT_NORMAL: &str = "layers/flat_normal.png";
pub const THUMBNAIL: &str = "thumbnail.png";
const THUMBNAIL_SIDE: u32 = 560;
/// Transparent margin around each layer, so filtering has room to fade out.
const MARGIN: f32 = 3.0;

/// Photos names the previews it hands out after a UUID, which means nothing to a person.
fn is_generated_name(stem: &str) -> bool {
    let mut parts = stem.split('-');
    let first = parts.next().unwrap_or_default();
    first.len() == 8 && first.chars().all(|c| c.is_ascii_hexdigit()) && parts.count() >= 4
}

/// A readable painting name from a file stem: `my_pothos-2` becomes `My pothos 2`.
pub fn painting_name(stem: &str) -> String {
    if is_generated_name(stem) {
        return "Untitled".into();
    }
    let spaced = stem.replace(['_', '-'], " ");
    let mut chars = spaced.trim().chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => "Untitled".into(),
    }
}

/// A safe folder name.
pub fn slug(name: &str) -> String {
    let kept: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    let joined = kept
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if joined.is_empty() {
        "untitled".into()
    } else {
        joined
    }
}

fn unused_folder(root: &Path, base: &str) -> PathBuf {
    let first = root.join(base);
    if !first.exists() {
        return first;
    }
    (2..)
        .map(|n| root.join(format!("{base}-{n}")))
        .find(|dir| !dir.exists())
        .expect("some suffix is free")
}

/// Decode upright: phones store photos sideways and record the turn in metadata.
fn decode_upright(path: &Path) -> Result<RgbaImage> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_lowercase();
    if matches!(extension.as_str(), "heic" | "heif") {
        bail!("HEIC photos aren't supported yet. Export it as JPEG or PNG and drop that instead.");
    }
    let mut decoder = ImageReader::open(path)
        .with_context(|| format!("opening {}", path.display()))?
        .with_guessed_format()?
        .into_decoder()
        .with_context(|| format!("{} isn't an image this app can read", path.display()))?;
    let orientation = decoder.orientation()?;
    let mut image = DynamicImage::from_decoder(decoder)?;
    image.apply_orientation(orientation);
    Ok(image.into_rgba8())
}

fn save_png(image: &RgbaImage, path: &Path) -> Result<()> {
    let file = File::create(path).with_context(|| format!("creating {}", path.display()))?;
    let encoder =
        PngEncoder::new_with_quality(BufWriter::new(file), CompressionType::Fast, PngFilter::Adaptive);
    image
        .write_with_encoder(encoder)
        .with_context(|| format!("writing {}", path.display()))
}

impl Project {
    /// Start a new painting in a fresh folder under `root`, from an image file.
    pub fn create(root: &Path, image: &Path) -> Result<Self> {
        let source = decode_upright(image)?;
        let stem = image.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
        let name = painting_name(stem);
        let dir = unused_folder(root, &slug(&name));
        std::fs::create_dir_all(dir.join("source")).with_context(|| format!("creating {}", dir.display()))?;
        save_png(&source, &dir.join(SOURCE))?;
        let project = Project {
            name,
            width: source.width(),
            height: source.height(),
            source: Some(SOURCE.into()),
            cuts: Vec::new(),
            layers: Vec::new(),
            dir,
        };
        project.save()?;
        project.write_thumbnail()?;
        Ok(project)
    }

    /// Rebuild every layer from the cuts. The first cut goes at the back.
    pub fn bake(&mut self) -> Result<()> {
        let source_path = self
            .source
            .as_ref()
            .context("this painting has no source image to cut")?;
        let source = image::open(self.dir.join(source_path))
            .with_context(|| format!("reading {}", source_path.display()))?
            .into_rgba8();
        let layers_dir = self.dir.join("layers");
        if layers_dir.exists() {
            std::fs::remove_dir_all(&layers_dir)?;
        }
        std::fs::create_dir_all(&layers_dir)?;
        RgbaImage::from_pixel(2, 2, image::Rgba([128, 128, 255, 255])).save(self.dir.join(FLAT_NORMAL))?;

        let mut layers = Vec::with_capacity(self.cuts.len());
        for (z, cut) in self.cuts.iter().enumerate() {
            let (x, y, piece) = cut_out(&source, cut);
            let color = PathBuf::from(format!("layers/z{z:02}_color.png"));
            save_png(&piece, &self.dir.join(&color))?;
            layers.push(Layer {
                name: cut.name.clone(),
                z: z as u32,
                x: x as f32,
                y: y as f32,
                w: piece.width(),
                h: piece.height(),
                color,
                normal: FLAT_NORMAL.into(),
            });
        }
        self.layers = layers;
        self.save()?;
        self.write_thumbnail()
    }

    /// What a card shows: the layers if there are any, otherwise the source image.
    pub fn thumbnail(&self) -> Result<RgbaImage> {
        let scale = THUMBNAIL_SIDE as f32 / self.width.max(self.height) as f32;
        let size = |n: u32| ((n as f32 * scale).round() as u32).max(1);
        if self.layers.is_empty() {
            let source = self
                .source
                .as_ref()
                .context("this painting has nothing to show")?;
            let image = image::open(self.dir.join(source))?.into_rgba8();
            return Ok(imageops::resize(
                &image,
                size(self.width),
                size(self.height),
                FilterType::Triangle,
            ));
        }
        let mut canvas = RgbaImage::new(size(self.width), size(self.height));
        for layer in &self.layers {
            let image = image::open(self.dir.join(&layer.color))?.into_rgba8();
            let piece = imageops::resize(&image, size(layer.w), size(layer.h), FilterType::Triangle);
            let (x, y) = ((layer.x * scale).round() as i64, (layer.y * scale).round() as i64);
            imageops::overlay(&mut canvas, &piece, x, y);
        }
        Ok(canvas)
    }

    pub fn write_thumbnail(&self) -> Result<()> {
        save_png(&self.thumbnail()?, &self.dir.join(THUMBNAIL))
    }
}

/// Copy a polygon out of the image, returning its position and pixels. Colour under
/// transparent pixels is kept, because the renderer filters across the edge.
fn cut_out(source: &RgbaImage, cut: &Cut) -> (u32, u32, RgbaImage) {
    let (mut x0, mut y0) = (f32::MAX, f32::MAX);
    let (mut x1, mut y1) = (f32::MIN, f32::MIN);
    for &[x, y] in &cut.points {
        (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
    }
    let left = (x0 - MARGIN).floor().max(0.0) as u32;
    let top = (y0 - MARGIN).floor().max(0.0) as u32;
    let right = ((x1 + MARGIN).ceil().max(0.0) as u32).clamp(left + 1, source.width().max(left + 1));
    let bottom = ((y1 + MARGIN).ceil().max(0.0) as u32).clamp(top + 1, source.height().max(top + 1));
    let (w, h) = (right - left, bottom - top);
    let mask = coverage(&cut.points, left, top, w, h);
    let mut piece = imageops::crop_imm(source, left, top, w, h).to_image();
    for (pixel, alpha) in piece.pixels_mut().zip(mask) {
        pixel[3] = ((pixel[3] as u32 * alpha as u32 + 127) / 255) as u8;
    }
    (left, top, piece)
}

/// Antialiased polygon coverage (even-odd) over a window of the image, one byte per pixel.
pub fn coverage(points: &[[f32; 2]], left: u32, top: u32, width: u32, height: u32) -> Vec<u8> {
    const ROWS: u32 = 4;
    let mut mask = Vec::with_capacity((width * height) as usize);
    let mut row = vec![0f32; width as usize];
    let mut crossings = Vec::new();
    for y in 0..height {
        row.fill(0.0);
        for sub in 0..ROWS {
            let sy = (top + y) as f32 + (sub as f32 + 0.5) / ROWS as f32;
            crossings.clear();
            for (i, a) in points.iter().enumerate() {
                let b = points[(i + 1) % points.len()];
                if (a[1] <= sy) != (b[1] <= sy) {
                    crossings.push(a[0] + (sy - a[1]) / (b[1] - a[1]) * (b[0] - a[0]));
                }
            }
            crossings.sort_by(f32::total_cmp);
            for [a, b] in crossings.as_chunks::<2>().0 {
                let (a, b) = (a - left as f32, b - left as f32);
                let first = a.floor().max(0.0) as usize;
                let last = (b.ceil().max(0.0) as usize).min(row.len());
                for (x, value) in row.iter_mut().enumerate().take(last).skip(first) {
                    *value += (b.min(x as f32 + 1.0) - a.max(x as f32)).max(0.0);
                }
            }
        }
        mask.extend(
            row.iter()
                .map(|v| (v / ROWS as f32 * 255.0).round().min(255.0) as u8),
        );
    }
    mask
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(points: &[[f32; 2]], w: u32, h: u32) -> f32 {
        coverage(points, 0, 0, w, h)
            .iter()
            .map(|a| *a as f32 / 255.0)
            .sum()
    }

    #[test]
    fn coverage_measures_area() {
        let square = [[2.0, 2.0], [8.0, 2.0], [8.0, 8.0], [2.0, 8.0]];
        assert!((area(&square, 10, 10) - 36.0).abs() < 0.01);
        let triangle = [[0.0, 0.0], [10.0, 0.0], [0.0, 10.0]];
        assert!((area(&triangle, 10, 10) - 50.0).abs() < 0.5);
    }

    #[test]
    fn coverage_handles_a_window_and_concave_shapes() {
        let notch = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [5.0, 5.0], [0.0, 10.0]];
        let mask = coverage(&notch, 4, 8, 2, 2);
        assert_eq!(mask.len(), 4);
        assert!(mask.iter().all(|a| *a < 40), "the notch is empty: {mask:?}");
        assert!((area(&notch, 10, 10) - 75.0).abs() < 0.5);
    }

    #[test]
    fn names_and_slugs_are_tidy() {
        assert_eq!(painting_name("my_pothos-2"), "My pothos 2");
        assert_eq!(painting_name(""), "Untitled");
        assert_eq!(
            painting_name("522387ad-d1a3-448b-9584-28369006a1b3-1-105-c"),
            "Untitled"
        );
        assert_eq!(painting_name("deadbeef-study"), "Deadbeef study");
        assert_eq!(slug("My pothos: 2!"), "my-pothos-2");
        assert_eq!(slug("!!"), "untitled");
    }

    #[test]
    fn a_dropped_image_becomes_a_baked_project() {
        let root = std::env::temp_dir().join(format!("las-bake-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let image = root.join("little_fern.png");
        RgbaImage::from_pixel(100, 80, image::Rgba([20, 120, 40, 255]))
            .save(&image)
            .unwrap();

        let mut project = Project::create(&root, &image).unwrap();
        assert_eq!(project.name, "Little fern");
        assert!(project.dir.ends_with("little-fern"));
        project.cuts = vec![
            Cut {
                name: "Back".into(),
                points: vec![[10.0, 10.0], [40.0, 10.0], [40.0, 40.0], [10.0, 40.0]],
            },
            Cut {
                name: "Front".into(),
                points: vec![[90.0, 70.0], [99.0, 70.0], [99.0, 79.0]],
            },
        ];
        project.bake().unwrap();

        let loaded = Project::load(&project.dir).unwrap();
        assert_eq!(loaded.cuts, project.cuts);
        assert_eq!(loaded.layers.len(), 2);
        let back = &loaded.layers[0];
        assert_eq!((back.x, back.y, back.w, back.h), (7.0, 7.0, 36, 36));
        let pixels = image::open(loaded.dir.join(&back.color)).unwrap().into_rgba8();
        assert_eq!(pixels.get_pixel(18, 18)[3], 255);
        assert_eq!(pixels.get_pixel(0, 0)[3], 0);
        assert!(loaded.dir.join(THUMBNAIL).is_file());

        assert!(
            Project::create(&root, &image)
                .unwrap()
                .dir
                .ends_with("little-fern-2")
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn heic_gets_a_clear_message() {
        let err = decode_upright(Path::new("photo.HEIC")).unwrap_err();
        assert!(err.to_string().contains("HEIC"));
    }
}
