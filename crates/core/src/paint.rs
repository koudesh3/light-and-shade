//! Painted normals. Strokes are the saved originals, in source pixels, so they survive
//! re-cutting; a normal map is only ever rebuilt from them.

use crate::{Layer, coverage};
use image::{Rgba, RgbaImage};
use serde::{Deserialize, Serialize};

/// Unpainted pixels face the viewer.
const FLAT: Rgba<u8> = Rgba([128, 128, 255, 255]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StrokeKind {
    Brush,
    Erase,
    /// Fill the area inside `points`.
    Shape,
    /// Fill the whole piece.
    Fill,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    pub kind: StrokeKind,
    /// x right, y up, z toward the viewer.
    pub normal: [f32; 3],
    pub radius: f32,
    pub points: Vec<[f32; 2]>,
}

/// The colour that stores a normal: each axis mapped from -1..1 to 0..255.
pub fn encode_normal(normal: [f32; 3]) -> Rgba<u8> {
    let len = normal.iter().map(|c| c * c).sum::<f32>().sqrt().max(1e-6);
    let [x, y, z] = normal.map(|c| ((c / len * 0.5 + 0.5) * 255.0).round() as u8);
    Rgba([x, y, z, 255])
}

/// The normal stored in a colour.
pub fn decode_normal(color: Rgba<u8>) -> [f32; 3] {
    let n = [0, 1, 2].map(|c| color[c] as f32 / 127.5 - 1.0);
    let len = n.iter().map(|c| c * c).sum::<f32>().sqrt().max(1e-6);
    n.map(|c| c / len)
}

/// One piece's normals, laid over its layer. There's no need to clip to the piece:
/// the colour layer's alpha already hides anything painted outside it.
pub struct NormalMap {
    left: f32,
    top: f32,
    pub image: RgbaImage,
}

impl NormalMap {
    pub fn new(layer: &Layer) -> Self {
        Self {
            left: layer.x,
            top: layer.y,
            image: RgbaImage::from_pixel(layer.w, layer.h, FLAT),
        }
    }

    pub fn from_strokes(layer: &Layer, strokes: &[Stroke]) -> Self {
        let mut map = Self::new(layer);
        strokes.iter().for_each(|stroke| map.apply(stroke));
        map
    }

    pub fn apply(&mut self, stroke: &Stroke) {
        let color = encode_normal(stroke.normal);
        match stroke.kind {
            StrokeKind::Fill => self.image.pixels_mut().for_each(|p| *p = color),
            StrokeKind::Brush | StrokeKind::Erase => {
                let color = if stroke.kind == StrokeKind::Erase {
                    FLAT
                } else {
                    color
                };
                let first = stroke.points.first().copied().unwrap_or_default();
                self.segment(first, first, stroke.radius, color);
                for pair in stroke.points.windows(2) {
                    self.segment(pair[0], pair[1], stroke.radius, color);
                }
            }
            StrokeKind::Shape => {
                let (w, h) = self.image.dimensions();
                let local: Vec<[f32; 2]> = stroke
                    .points
                    .iter()
                    .map(|p| [p[0] - self.left, p[1] - self.top])
                    .collect();
                for (pixel, covered) in self.image.pixels_mut().zip(coverage(&local, 0, 0, w, h)) {
                    if covered >= 128 {
                        *pixel = color;
                    }
                }
            }
        }
    }

    /// Paint a round-ended line between two source-pixel points.
    pub fn segment(&mut self, a: [f32; 2], b: [f32; 2], radius: f32, color: Rgba<u8>) {
        let (ax, ay) = (a[0] - self.left, a[1] - self.top);
        let (bx, by) = (b[0] - self.left, b[1] - self.top);
        let (w, h) = self.image.dimensions();
        let x0 = (ax.min(bx) - radius).floor().max(0.0) as u32;
        let y0 = (ay.min(by) - radius).floor().max(0.0) as u32;
        let x1 = ((ax.max(bx) + radius).ceil().max(0.0) as u32).min(w);
        let y1 = ((ay.max(by) + radius).ceil().max(0.0) as u32).min(h);
        let (dx, dy) = (bx - ax, by - ay);
        let length2 = (dx * dx + dy * dy).max(1e-6);
        for y in y0..y1 {
            for x in x0..x1 {
                let (px, py) = (x as f32 + 0.5 - ax, y as f32 + 0.5 - ay);
                let t = ((px * dx + py * dy) / length2).clamp(0.0, 1.0);
                let (ex, ey) = (px - t * dx, py - t * dy);
                if ex * ex + ey * ey <= radius * radius {
                    self.image.put_pixel(x, y, color);
                }
            }
        }
    }

    /// The normal at a source-pixel point, if it's on this piece.
    pub fn sample(&self, point: [f32; 2]) -> Option<[f32; 3]> {
        let (x, y) = (point[0] - self.left, point[1] - self.top);
        (x >= 0.0 && y >= 0.0 && (x as u32) < self.image.width() && (y as u32) < self.image.height())
            .then(|| decode_normal(*self.image.get_pixel(x as u32, y as u32)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn layer() -> Layer {
        Layer {
            name: "Leaf".into(),
            z: 0,
            x: 100.0,
            y: 200.0,
            w: 40,
            h: 30,
            color: PathBuf::new(),
            normal: PathBuf::new(),
        }
    }

    fn stroke(kind: StrokeKind, points: Vec<[f32; 2]>) -> Stroke {
        Stroke {
            kind,
            normal: [1.0, 0.0, 0.0],
            radius: 5.0,
            points,
        }
    }

    #[test]
    fn normals_round_trip_through_colour() {
        let color = encode_normal([0.6, -0.48, 0.64]);
        let n = decode_normal(color);
        for (a, b) in n.iter().zip([0.6, -0.48, 0.64]) {
            assert!((a - b).abs() < 0.01, "{n:?}");
        }
    }

    #[test]
    fn a_brush_paints_near_its_line_in_layer_space() {
        let map = NormalMap::from_strokes(
            &layer(),
            &[stroke(StrokeKind::Brush, vec![[110.0, 215.0], [130.0, 215.0]])],
        );
        let red = encode_normal([1.0, 0.0, 0.0]);
        assert_eq!(*map.image.get_pixel(20, 15), red);
        assert_eq!(*map.image.get_pixel(10, 15), red);
        assert_eq!(*map.image.get_pixel(20, 2), FLAT);
    }

    #[test]
    fn a_single_click_leaves_a_dot() {
        let map = NormalMap::from_strokes(&layer(), &[stroke(StrokeKind::Brush, vec![[110.0, 215.0]])]);
        assert_eq!(*map.image.get_pixel(10, 15), encode_normal([1.0, 0.0, 0.0]));
    }

    #[test]
    fn erase_returns_to_flat() {
        let map = NormalMap::from_strokes(
            &layer(),
            &[
                stroke(StrokeKind::Fill, vec![]),
                stroke(StrokeKind::Erase, vec![[110.0, 215.0]]),
            ],
        );
        assert_eq!(*map.image.get_pixel(10, 15), FLAT);
        assert_eq!(*map.image.get_pixel(30, 15), encode_normal([1.0, 0.0, 0.0]));
    }

    #[test]
    fn a_shape_fills_only_inside() {
        let square = vec![[105.0, 205.0], [115.0, 205.0], [115.0, 215.0], [105.0, 215.0]];
        let map = NormalMap::from_strokes(&layer(), &[stroke(StrokeKind::Shape, square)]);
        assert_eq!(*map.image.get_pixel(10, 10), encode_normal([1.0, 0.0, 0.0]));
        assert_eq!(*map.image.get_pixel(30, 10), FLAT);
    }

    #[test]
    fn strokes_outside_the_layer_are_harmless() {
        let far = stroke(StrokeKind::Brush, vec![[-500.0, -500.0], [-400.0, -400.0]]);
        let map = NormalMap::from_strokes(&layer(), &[far]);
        assert!(map.image.pixels().all(|p| *p == FLAT));
        assert_eq!(map.sample([0.0, 0.0]), None);
    }
}
