//! The light ball: a shaded sphere you drag to aim the light.

use eframe::egui::{self, Color32, ColorImage, Sense, TextureHandle, TextureOptions, Vec2};
use las_core::Light;

/// How far out the light sits when dragged to the ball's rim.
const RIM: f32 = 1.6;

#[derive(Default)]
pub struct Ball {
    texture: Option<TextureHandle>,
    drawn: Option<(Light, f32, f32, f32, u32)>,
}

impl Ball {
    pub fn show(&mut self, ui: &mut egui::Ui, light: &mut Light, ambient: f32, strength: f32, sheen: f32) -> bool {
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(112.0), Sense::click_and_drag());
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
                None => self.texture = Some(ui.ctx().load_texture("light ball", image, TextureOptions::LINEAR)),
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
        ui.painter().circle_stroke(dot, 4.0, (1.5, Color32::from_rgba_unmultiplied(255, 255, 255, 210)));
        changed
    }
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
            let v = ((ambient + strength * dif) * 170.0 + spec * 255.0).min(255.0) * edge;
            pixels[y * size + x] =
                Color32::from_rgba_premultiplied((v * 0.92) as u8, v as u8, (v * 0.9) as u8, (edge * 255.0) as u8);
        }
    }
    ColorImage::new([size, size], pixels)
}
