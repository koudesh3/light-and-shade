//! The light ball: a shaded sphere you drag to aim the light.

use eframe::egui::{self, Color32, ColorImage, Sense, TextureHandle, TextureOptions, Vec2};
use las_core::{Light, direction_palette, encode_normal, nearest_direction};

/// How far out the light sits when dragged to the ball's rim.
const RIM: f32 = 1.6;

#[derive(Default)]
pub struct Ball {
    texture: Option<TextureHandle>,
    drawn: Option<(Light, f32, f32, f32, u32)>,
}

impl Ball {
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        light: &mut Light,
        ambient: f32,
        strength: f32,
        sheen: f32,
    ) -> bool {
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(96.0), Sense::click_and_drag());
        let mut changed = false;
        if let Some(pos) = response.interact_pointer_pos() {
            let v = (pos - rect.center()) / (rect.width() / 2.0);
            let m = v.length().max(1.0);
            light.x = v.x / m * RIM;
            light.y = -v.y / m * RIM;
            changed = true;
        }
        let px = (rect.width() * ui.pixels_per_point()).round() as u32;
        let key = (*light, ambient, strength, sheen, px);
        if self.drawn != Some(key) || self.texture.is_none() {
            let image = shade(px as usize, light.direction(), ambient, strength, sheen);
            match &mut self.texture {
                Some(t) => t.set(image, TextureOptions::LINEAR),
                None => {
                    self.texture = Some(ui.ctx().load_texture("light ball", image, TextureOptions::LINEAR))
                }
            }
            self.drawn = Some(key);
        }
        if let Some(t) = &self.texture {
            ui.painter().image(
                t.id(),
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        let d = light.direction();
        let dot = rect.center() + egui::vec2(d[0], -d[1]) * rect.width() / 2.0;
        ui.painter().circle_stroke(
            dot,
            4.0,
            (1.5, Color32::from_rgba_unmultiplied(255, 255, 255, 210)),
        );
        changed
    }
}

/// The direction sphere: every facing direction, drawn in its normal-map colour. Given a
/// number of shades, it divides into that many Voronoi cells and picking snaps to them.
#[derive(Default)]
pub struct Sphere {
    texture: Option<(TextureHandle, Option<usize>)>,
}

impl Sphere {
    pub fn show(&mut self, ui: &mut egui::Ui, normal: &mut [f32; 3], shades: Option<usize>) -> bool {
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(132.0), Sense::click_and_drag());
        let palette = shades.map(direction_palette);
        let mut changed = false;
        if let Some(pos) = response.interact_pointer_pos() {
            let v = (pos - rect.center()) / (rect.width() / 2.0);
            let m = (v.length() / 0.98).max(1.0);
            let (x, y) = (v.x / m, -v.y / m);
            *normal = [x, y, (1.0 - x * x - y * y).max(0.0).sqrt()];
            changed = true;
        }
        if let Some(palette) = &palette {
            *normal = nearest_direction(palette, *normal);
        }
        if self.texture.as_ref().is_none_or(|(_, drawn)| *drawn != shades) {
            let px = (rect.width() * ui.pixels_per_point()).round() as usize;
            let image = directions(px, palette.as_deref());
            let texture = ui
                .ctx()
                .load_texture("direction sphere", image, TextureOptions::LINEAR);
            self.texture = Some((texture, shades));
        }
        if let Some((texture, _)) = &self.texture {
            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            ui.painter().image(texture.id(), rect, uv, Color32::WHITE);
        }
        let at = rect.center() + egui::vec2(normal[0], -normal[1]) * rect.width() / 2.0;
        ui.painter().circle_stroke(at, 5.0, (2.5, Color32::WHITE));
        ui.painter().circle_stroke(at, 5.0, (1.0, Color32::BLACK));
        changed
    }
}

fn directions(size: usize, palette: Option<&[[f32; 3]]>) -> ColorImage {
    /// Samples per pixel along each axis, which keeps the cell borders smooth.
    const GRID: usize = 3;
    let r = size as f32 / 2.0;
    let mut pixels = vec![Color32::TRANSPARENT; size * size];
    for y in 0..size {
        for x in 0..size {
            let (mut sum, mut covered) = ([0.0f32; 3], 0);
            for (sx, sy) in (0..GRID * GRID).map(|i| (i % GRID, i / GRID)) {
                let nx = (x as f32 + (sx as f32 + 0.5) / GRID as f32 - r) / r;
                let ny = -(y as f32 + (sy as f32 + 0.5) / GRID as f32 - r) / r;
                let r2 = nx * nx + ny * ny;
                if r2 >= 1.0 {
                    continue;
                }
                let n = [nx, ny, (1.0 - r2).sqrt()];
                let color = encode_normal(palette.map_or(n, |p| nearest_direction(p, n)));
                (0..3).for_each(|c| sum[c] += color[c] as f32);
                covered += 1;
            }
            if covered > 0 {
                let channel = |c: f32| (c / (GRID * GRID) as f32) as u8;
                let alpha = 255 * covered / (GRID * GRID);
                pixels[y * size + x] = Color32::from_rgba_premultiplied(
                    channel(sum[0]),
                    channel(sum[1]),
                    channel(sum[2]),
                    alpha as u8,
                );
            }
        }
    }
    ColorImage::new([size, size], pixels)
}

fn shade(size: usize, l: [f32; 3], ambient: f32, strength: f32, sheen: f32) -> ColorImage {
    let r = size as f32 / 2.0;
    let h = {
        let v = [l[0], l[1], l[2] + 1.0];
        let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-6);
        [v[0] / n, v[1] / n, v[2] / n]
    };
    let mut pixels = vec![Color32::TRANSPARENT; size * size];
    for y in 0..size {
        for x in 0..size {
            let (nx, ny) = ((x as f32 + 0.5 - r) / r, -(y as f32 + 0.5 - r) / r);
            let r2 = nx * nx + ny * ny;
            let edge = ((1.0 - r2.sqrt()) * r).clamp(0.0, 1.0);
            if edge <= 0.0 {
                continue;
            }
            let nz = (1.0 - r2).max(0.0).sqrt();
            let dif = (nx * l[0] + ny * l[1] + nz * l[2]).max(0.0);
            let spec = (nx * h[0] + ny * h[1] + nz * h[2]).max(0.0).powf(28.0) * sheen;
            let v = (((ambient + strength * dif) * 170.0 + spec * 255.0).min(255.0) * edge) as u8;
            pixels[y * size + x] = Color32::from_rgba_premultiplied(v, v, v, (edge * 255.0) as u8);
        }
    }
    ColorImage::new([size, size], pixels)
}
