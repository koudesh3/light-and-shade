//! A quiet dark theme, so the UI stays out of the way of the painting's colour.

use eframe::egui::{self, Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle, Vec2};

pub const PANEL: Color32 = Color32::from_rgb(38, 39, 41);
pub const WINDOW: Color32 = Color32::from_rgb(30, 31, 33);
pub const LINE: Color32 = Color32::from_rgb(20, 20, 22);
pub const TEXT: Color32 = Color32::from_rgb(214, 215, 217);
pub const MUTED: Color32 = Color32::from_rgb(142, 144, 148);
pub const ACCENT: Color32 = Color32::from_rgb(92, 142, 110);

pub fn apply(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    let mut v = egui::Visuals::dark();
    v.panel_fill = PANEL;
    v.window_fill = PANEL;
    v.extreme_bg_color = WINDOW;
    v.faint_bg_color = Color32::from_rgb(44, 45, 47);
    v.override_text_color = Some(TEXT);
    v.selection.bg_fill = ACCENT;
    v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    v.slider_trailing_fill = true;
    let radius = CornerRadius::same(5);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = radius;
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(56, 57, 60);
    v.widgets.inactive.bg_fill = Color32::from_rgb(66, 67, 70);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(68, 69, 72);
    v.widgets.active.weak_bg_fill = Color32::from_rgb(78, 79, 83);
    ctx.set_visuals_of(egui::Theme::Dark, v);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = Vec2::new(8.0, 6.0);
        s.spacing.button_padding = Vec2::new(9.0, 3.0);
        s.spacing.interact_size.y = 22.0;
        s.spacing.slider_width = 140.0;
        s.text_styles = [
            (TextStyle::Small, FontId::new(10.5, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(12.5, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(12.5, FontFamily::Proportional)),
            (TextStyle::Heading, FontId::new(13.0, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(11.5, FontFamily::Monospace)),
        ]
        .into();
    });
}

/// A small caps-style section label.
pub fn section(ui: &mut egui::Ui, text: &str) {
    ui.add_space(4.0);
    ui.label(egui::RichText::new(text.to_uppercase()).size(10.0).strong().color(MUTED));
}
