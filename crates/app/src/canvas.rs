//! Moving around a painting, shared by every screen that shows one.

use eframe::egui::{self, Event, MouseWheelUnit, Pos2, Rect};
use las_core::View;

/// Whether undo or redo was pressed this frame. Both Control and Command work.
pub fn history_keys(input: &egui::InputState) -> (bool, bool) {
    let held = input.modifiers.command || input.modifiers.ctrl;
    let z = held && input.key_pressed(egui::Key::Z);
    (z && !input.modifiers.shift, z && input.modifiers.shift)
}

/// Each notch of a mouse wheel zooms by this much, as an exponent.
const WHEEL_ZOOM: f32 = 0.08;

/// Apply this frame's scrolling while the pointer is at `pointer` over the canvas. Two fingers
/// on a trackpad pan, and a pinch or a mouse wheel zooms. Trackpads report precise point
/// deltas and wheels report notches, which tells them apart on every platform.
pub fn navigate(ui: &egui::Ui, view: &mut View, pointer: Pos2, rect: Rect, painting: [f32; 2]) {
    let ppp = ui.pixels_per_point();
    let target = [
        (rect.width() * ppp).round().max(1.0),
        (rect.height() * ppp).round().max(1.0),
    ];
    let (mut pan, mut factor) = (egui::Vec2::ZERO, ui.input(|i| i.zoom_delta()));
    ui.input(|i| {
        for event in &i.events {
            if let Event::MouseWheel {
                unit,
                delta,
                modifiers,
                ..
            } = event
                && !modifiers.command
            {
                match unit {
                    MouseWheelUnit::Point => pan += *delta,
                    MouseWheelUnit::Line | MouseWheelUnit::Page => {
                        factor *= (delta.y * WHEEL_ZOOM).exp();
                    }
                }
            }
        }
    });
    view.pan[0] += pan.x * ppp;
    view.pan[1] += pan.y * ppp;
    if factor != 1.0 {
        let at = (pointer - rect.min) * ppp;
        view.zoom_at(factor, [at.x, at.y], target, painting);
    }
}

/// Where the painting sits on screen. Everything that draws over a painting or reads clicks
/// from it goes through this, so the two can't disagree. `View` works in device pixels and
/// the UI in points, and they only match on a 1x display.
#[derive(Clone, Copy)]
pub struct Placement {
    rect: Rect,
    scale: f32,
    origin: [f32; 2],
}

impl Placement {
    pub fn of(ui: &egui::Ui, view: &View, rect: Rect, painting: [f32; 2]) -> Self {
        Self::at_scale(ui.pixels_per_point(), view, rect, painting)
    }

    fn at_scale(ppp: f32, view: &View, rect: Rect, painting: [f32; 2]) -> Self {
        let target = [
            (rect.width() * ppp).round().max(1.0),
            (rect.height() * ppp).round().max(1.0),
        ];
        let (scale, origin) = (view.scale(target, painting), view.origin(target, painting));
        Self {
            rect,
            scale: scale / ppp,
            origin: [origin[0] / ppp, origin[1] / ppp],
        }
    }

    /// Screen points per painting pixel.
    pub fn scale(self) -> f32 {
        self.scale
    }

    pub fn to_screen(self, p: [f32; 2]) -> Pos2 {
        self.rect.min
            + egui::vec2(
                self.origin[0] + p[0] * self.scale,
                self.origin[1] + p[1] * self.scale,
            )
    }

    pub fn to_painting(self, p: Pos2) -> [f32; 2] {
        let v = p - self.rect.min;
        [
            (v.x - self.origin[0]) / self.scale,
            (v.y - self.origin[1]) / self.scale,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::pos2;

    #[test]
    fn clicks_land_where_the_painting_is_drawn_after_panning_on_retina() {
        let rect = Rect::from_min_max(pos2(216.0, 46.0), pos2(1360.0, 880.0));
        let painting = [3024.0, 4032.0];
        let view = View {
            zoom: 1.5,
            pan: [-492.0, -427.0],
            frame: Some([400.0, 900.0, 2600.0, 3300.0]),
        };
        for ppp in [1.0, 2.0] {
            let placed = Placement::at_scale(ppp, &view, rect, painting);
            let click = pos2(600.0, 300.0);
            let back = placed.to_screen(placed.to_painting(click));
            assert!(back.distance(click) < 0.01, "{back:?} at {ppp}x");

            // The same point must match what the renderer draws, in device pixels.
            let p = placed.to_painting(click);
            let target = [(rect.width() * ppp).round(), (rect.height() * ppp).round()];
            let (s, o) = (view.scale(target, painting), view.origin(target, painting));
            let device = [o[0] + p[0] * s, o[1] + p[1] * s];
            let expected = (click - rect.min) * ppp;
            assert!((device[0] - expected.x).abs() < 0.01 && (device[1] - expected.y).abs() < 0.01);
        }
    }
}
