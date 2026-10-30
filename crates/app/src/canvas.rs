//! Moving around a painting, shared by every screen that shows one.

use eframe::egui::{self, Event, MouseWheelUnit, Pos2, Rect};
use las_core::View;

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

/// Where the painting sits, in points within `rect`: its scale and its top-left corner.
pub fn placement(ui: &egui::Ui, view: &View, rect: Rect, painting: [f32; 2]) -> (f32, [f32; 2]) {
    let ppp = ui.pixels_per_point();
    let target = [
        (rect.width() * ppp).round().max(1.0),
        (rect.height() * ppp).round().max(1.0),
    ];
    let (scale, origin) = (view.scale(target, painting), view.origin(target, painting));
    (scale / ppp, [origin[0] / ppp, origin[1] / ppp])
}
