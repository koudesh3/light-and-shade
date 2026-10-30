//! The design system: white surfaces, black Inter, squircles, and nothing else.
//! Every control in the app comes from here, so a change here changes the whole app.

use eframe::egui::{
    self, Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, Pos2, Rect, Response, Sense, Shape,
    Stroke, Ui, Vec2, pos2, vec2,
};
use std::ops::RangeInclusive;
use std::sync::Arc;

pub const WHITE: Color32 = Color32::WHITE;
pub const INK: Color32 = Color32::from_rgb(17, 17, 17);
pub const MUTED: Color32 = Color32::from_rgb(138, 138, 134);
/// Hover and selection, the only grey fill in the app.
pub const WASH: Color32 = Color32::from_rgb(242, 242, 240);
pub const RAIL: Color32 = Color32::from_rgb(228, 228, 225);

pub const RADIUS: f32 = 8.0;
pub const ROW: f32 = 28.0;
pub const GAP: f32 = 6.0;
pub const PAGE: i8 = 14;

const TEXT: f32 = 13.0;
const HEADING: f32 = 12.0;

fn regular(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("medium".into()))
}

pub fn apply(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let data = [
        ("Inter", &include_bytes!("../assets/fonts/Inter-Regular.ttf")[..]),
        (
            "Inter Medium",
            &include_bytes!("../assets/fonts/Inter-Medium.ttf")[..],
        ),
    ];
    for (name, bytes) in data {
        fonts
            .font_data
            .insert(name.into(), Arc::new(FontData::from_static(bytes)));
    }
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "Inter".into());
    fonts
        .families
        .insert(FontFamily::Name("medium".into()), vec!["Inter Medium".into()]);
    ctx.set_fonts(fonts);

    ctx.set_theme(egui::Theme::Light);
    let mut v = egui::Visuals::light();
    v.panel_fill = WHITE;
    v.window_fill = WHITE;
    v.window_stroke = Stroke::new(1.0, RAIL);
    v.window_corner_radius = egui::CornerRadius::same(RADIUS as u8);
    v.override_text_color = Some(INK);
    v.selection.bg_fill = WASH;
    v.selection.stroke = Stroke::new(1.0, INK);
    ctx.set_visuals_of(egui::Theme::Light, v);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = vec2(GAP, GAP);
        s.text_styles.insert(egui::TextStyle::Body, regular(TEXT));
        s.text_styles.insert(egui::TextStyle::Button, regular(TEXT));
    });
}

/// A superellipse-cornered rectangle. Unlike a circular corner, its curve eases
/// into the straight edges, which is what makes it read as soft rather than round.
pub fn squircle(rect: Rect, radius: f32, fill: Color32) -> Shape {
    const STEPS: usize = 12;
    const POWER: f32 = 2.0 / 4.5;
    // A superellipse looks tighter than a circle of the same size, so reach further along the edges.
    let r = (radius * 1.5).min(rect.width() / 2.0).min(rect.height() / 2.0);
    let corners = [
        (pos2(rect.right() - r, rect.top() + r), 1.0, -1.0, true),
        (pos2(rect.right() - r, rect.bottom() - r), 1.0, 1.0, false),
        (pos2(rect.left() + r, rect.bottom() - r), -1.0, 1.0, true),
        (pos2(rect.left() + r, rect.top() + r), -1.0, -1.0, false),
    ];
    let mut points = Vec::with_capacity((STEPS + 1) * 4);
    for (center, sx, sy, up) in corners {
        for i in 0..=STEPS {
            let t = i as f32 / STEPS as f32;
            let a = std::f32::consts::FRAC_PI_2 * if up { t } else { 1.0 - t };
            // Walk each corner clockwise, from the end of one edge to the start of the next.
            let (x, y) = (a.sin().max(0.0).powf(POWER), a.cos().max(0.0).powf(POWER));
            points.push(center + vec2(sx * r * x, sy * r * y));
        }
    }
    Shape::convex_polygon(points, fill, Stroke::NONE)
}

/// A quiet label above a group of controls.
pub fn heading(ui: &mut Ui, text: &str) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::hover());
    ui.painter().text(
        rect.left_center(),
        Align2::LEFT_CENTER,
        text,
        medium(HEADING),
        MUTED,
    );
}

/// Air between groups.
pub fn gap(ui: &mut Ui) {
    ui.add_space(18.0);
}

fn hover_fill(ui: &Ui, response: &Response, selected: bool) -> Color32 {
    let t = ui
        .ctx()
        .animate_bool_with_time(response.id, response.hovered() || selected, 0.1);
    WHITE.lerp_to_gamma(WASH, t)
}

/// A full-width row, as in a sidebar.
pub fn row(ui: &mut Ui, text: &str, selected: bool, color: Color32) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), ROW), Sense::click());
    ui.painter()
        .add(squircle(rect, RADIUS, hover_fill(ui, &response, selected)));
    let font = if selected { medium(TEXT) } else { regular(TEXT) };
    let at = rect.left_center() + vec2(8.0, 0.0);
    ui.painter().text(at, Align2::LEFT_CENTER, text, font, color);
    response
}

/// A row that switches something on and off, with a dot showing which.
pub fn layer_row(ui: &mut Ui, text: &str, on: bool) -> Response {
    let response = row(ui, text, false, if on { INK } else { MUTED });
    let dot = response.rect.right_center() - vec2(13.0, 0.0);
    if on {
        ui.painter().circle_filled(dot, 3.0, INK);
    } else {
        ui.painter().circle_stroke(dot, 2.5, Stroke::new(1.0, MUTED));
    }
    response
}

fn width_of(ui: &Ui, text: &str, font: FontId) -> f32 {
    ui.fonts_mut(|f| f.layout_no_wrap(text.to_owned(), font, INK).size().x)
}

fn pill(ui: &mut Ui, rect: Rect, text: &str, selected: bool) -> Response {
    let response = ui.interact(rect, ui.scope_id().with(text), Sense::click());
    ui.painter()
        .add(squircle(rect, RADIUS, hover_fill(ui, &response, selected)));
    let color = if selected { INK } else { MUTED };
    let font = if selected { medium(TEXT) } else { regular(TEXT) };
    ui.painter()
        .text(rect.center(), Align2::CENTER_CENTER, text, font, color);
    response
}

/// A small text button.
pub fn button(ui: &mut Ui, text: &str) -> Response {
    let size = vec2(width_of(ui, text, regular(TEXT)) + 20.0, ROW);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    pill(ui, rect, text, false)
}

/// One-of-many tabs, centred on `center`. Returns the index that was clicked.
pub fn tabs(ui: &mut Ui, center: Pos2, labels: &[&str], selected: usize) -> Option<usize> {
    let widths: Vec<f32> = labels
        .iter()
        .map(|l| width_of(ui, l, medium(TEXT)) + 24.0)
        .collect();
    let total = widths.iter().sum::<f32>() + GAP * (labels.len() - 1) as f32;
    let mut x = center.x - total / 2.0;
    let mut clicked = None;
    for (i, (label, width)) in labels.iter().zip(widths).enumerate() {
        let rect = Rect::from_min_size(pos2(x, center.y - ROW / 2.0), vec2(width, ROW));
        if pill(ui, rect, label, i == selected).clicked() {
            clicked = Some(i);
        }
        x += width + GAP;
    }
    clicked
}

/// A labelled slider: name and value above a hairline rail with a black knob.
pub fn slider(ui: &mut Ui, label: &str, value: &mut f32, range: RangeInclusive<f32>) -> bool {
    const KNOB: f32 = 6.0;
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 42.0), Sense::click_and_drag());
    let (lo, hi) = (*range.start(), *range.end());
    let rail_y = rect.bottom() - 9.0;
    let (left, right) = (rect.left() + KNOB, rect.right() - KNOB);
    let mut changed = false;
    if let Some(p) = response.interact_pointer_pos() {
        let t = ((p.x - left) / (right - left)).clamp(0.0, 1.0);
        let next = lo + t * (hi - lo);
        changed = next != *value;
        *value = next;
    }
    let x = left + (*value - lo) / (hi - lo) * (right - left);
    let painter = ui.painter();
    let top = rect.top() + 9.0;
    painter.text(
        pos2(rect.left(), top),
        Align2::LEFT_CENTER,
        label,
        regular(TEXT),
        INK,
    );
    painter.text(
        pos2(rect.right(), top),
        Align2::RIGHT_CENTER,
        format!("{value:.2}"),
        regular(TEXT),
        MUTED,
    );
    let line = |a: f32, b: f32, color| painter.line_segment([pos2(a, rail_y), pos2(b, rail_y)], (1.5, color));
    line(rect.left(), rect.right(), RAIL);
    line(rect.left(), x, INK);
    painter.circle_filled(pos2(x, rail_y), KNOB, INK);
    painter.circle_filled(pos2(x, rail_y), KNOB - 1.5, WHITE);
    changed
}

/// A labelled on/off switch.
pub fn toggle(ui: &mut Ui, label: &str, on: &mut bool) -> Response {
    let (rect, mut response) = ui.allocate_exact_size(vec2(ui.available_width(), ROW), Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    let t = ui.ctx().animate_bool_with_time(response.id, *on, 0.12);
    let size = Vec2::new(30.0, 18.0);
    let track = Rect::from_center_size(rect.right_center() - vec2(size.x / 2.0, 0.0), size);
    let painter = ui.painter();
    painter.text(rect.left_center(), Align2::LEFT_CENTER, label, regular(TEXT), INK);
    painter.add(squircle(track, size.y / 2.0, RAIL.lerp_to_gamma(INK, t)));
    let travel = size.x - size.y;
    let knob = track.left_center() + vec2(size.y / 2.0 + t * travel, 0.0);
    painter.circle_filled(knob, 7.0, WHITE);
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn squircle_stays_finite_and_inside_its_rect() {
        let rect = Rect::from_min_size(pos2(10.0, 20.0), vec2(120.0, 28.0));
        let Shape::Path(path) = squircle(rect, RADIUS, INK) else {
            panic!("expected a path")
        };
        for p in &path.points {
            assert!(p.x.is_finite() && p.y.is_finite());
            assert!(rect.expand(0.01).contains(*p), "{p:?}");
        }
    }
}
