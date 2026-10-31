//! The design system: white surfaces, black Inter, squircles, and nothing else.
//! Every control in the app comes from here, so a change here changes the whole app.

use eframe::egui::text::LayoutJob;
use eframe::egui::{
    self, Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, Pos2, Rect, Response, Sense, Shape,
    Stroke, Ui, Vec2, pos2, vec2,
};
use std::ops::RangeInclusive;
use std::sync::Arc;

pub const WHITE: Color32 = Color32::WHITE;
/// The window behind everything. Controls and the canvas sit on white squircles above it.
pub const SHELL: Color32 = Color32::from_rgb(236, 236, 233);
pub const INK: Color32 = Color32::from_rgb(17, 17, 17);
pub const MUTED: Color32 = Color32::from_rgb(138, 138, 134);
/// Hover and selection, the only grey fill in the app.
pub const WASH: Color32 = Color32::from_rgb(242, 242, 240);
pub const RAIL: Color32 = Color32::from_rgb(228, 228, 225);

/// A strip above the toolbar for the window buttons, which float there on macOS.
const WINDOW_BUTTONS: f32 = if cfg!(target_os = "macos") { 28.0 } else { 0.0 };
pub const RADIUS: f32 = 8.0;
pub const ROW: f32 = 28.0;
pub const GAP: f32 = 6.0;
pub const PAGE: i8 = 14;
/// How far a tray reaches past the controls on it.
const TRAY_EDGE: f32 = 3.0;

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
    v.panel_fill = SHELL;
    v.window_fill = WHITE;
    v.window_stroke = Stroke::new(1.0, RAIL);
    v.window_corner_radius = egui::CornerRadius::same(RADIUS as u8);
    v.override_text_color = Some(INK);
    v.selection.bg_fill = WASH;
    v.selection.stroke = Stroke::new(1.0, INK);
    ctx.set_visuals_of(egui::Theme::Light, v);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = vec2(GAP, GAP);
        s.spacing.scroll = egui::style::ScrollStyle::floating();
        s.text_styles.insert(egui::TextStyle::Body, regular(TEXT));
        s.text_styles.insert(egui::TextStyle::Button, regular(TEXT));
    });
}

/// Line segments in each corner of a squircle.
const CORNER_STEPS: usize = 12;
/// Corner radius of the big surfaces.
const SURFACE: f32 = RADIUS * 1.75;

/// A superellipse-cornered rectangle. Unlike a circular corner, its curve eases
/// into the straight edges, which is what makes it read as soft rather than round.
pub fn squircle(rect: Rect, radius: f32, fill: Color32) -> Shape {
    Shape::convex_polygon(squircle_points(rect, radius), fill, Stroke::NONE)
}

/// A squircle's outline.
pub fn squircle_outline(rect: Rect, radius: f32, stroke: Stroke) -> Shape {
    Shape::closed_line(squircle_points(rect, radius), stroke)
}

fn squircle_points(rect: Rect, radius: f32) -> Vec<Pos2> {
    const STEPS: usize = CORNER_STEPS;
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
    points
}

/// A frame for what sits directly on the window.
pub fn shell() -> egui::Frame {
    egui::Frame::new().fill(SHELL)
}

/// The frame around a canvas: the space left by the side panels, down to the window's edge.
pub fn canvas_frame(panel_on_right: bool) -> egui::Frame {
    shell().inner_margin(egui::Margin {
        left: 0,
        right: if panel_on_right { 0 } else { PAGE },
        top: 2,
        bottom: PAGE,
    })
}

/// Paint a canvas's white surface.
pub fn canvas_surface(ui: &Ui, rect: Rect) {
    ui.painter().add(squircle(rect, SURFACE, WHITE));
}

/// Round off a canvas by covering its corners, over whatever has been drawn on it.
pub fn canvas_corners(ui: &Ui, rect: Rect) {
    let outer = [
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
        rect.left_top(),
    ];
    let points = squircle_points(rect, SURFACE);
    for (corner, arc) in outer.iter().zip(points.chunks(CORNER_STEPS + 1)) {
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(*corner, SHELL);
        arc.iter().for_each(|p| mesh.colored_vertex(*p, SHELL));
        for i in 1..arc.len() as u32 {
            mesh.add_triangle(0, i, i + 1);
        }
        ui.painter().add(Shape::mesh(mesh));
    }
    // The covers have hard edges, so a thin line softens where they meet the canvas.
    ui.painter()
        .add(Shape::closed_line(points, Stroke::new(1.0, SHELL)));
}

/// A group of controls on its own white squircle, as wide as the space it's in.
pub fn group<R>(ui: &mut Ui, contents: impl FnOnce(&mut Ui) -> R) -> R {
    let background = ui.painter().add(Shape::Noop);
    let inner = egui::Frame::new()
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            contents(ui)
        });
    ui.painter()
        .set(background, squircle(inner.response.rect, SURFACE, WHITE));
    inner.inner
}

/// A row of toolbar controls on a small white squircle, hugging what's inside.
pub fn tray<R>(ui: &mut Ui, contents: impl FnOnce(&mut Ui) -> R) -> R {
    let background = ui.painter().add(Shape::Noop);
    let inner = ui.scope(contents);
    // A tray can fill its whole row, so its height comes from the controls, not the row.
    let rect = Rect::from_center_size(
        inner.response.rect.center(),
        vec2(inner.response.rect.width(), ROW),
    );
    let rect = rect.expand(TRAY_EDGE);
    ui.painter().set(background, squircle(rect, RADIUS * 1.25, WHITE));
    inner.inner
}

/// The bar along the top of a screen.
pub fn top_bar<R>(ui: &mut Ui, contents: impl FnOnce(&mut Ui) -> R) -> R {
    let frame = shell().inner_margin(egui::Margin {
        left: PAGE + TRAY_EDGE as i8,
        right: PAGE + TRAY_EDGE as i8,
        top: WINDOW_BUTTONS as i8,
        bottom: 0,
    });
    egui::Panel::top("top bar")
        .exact_size(WINDOW_BUTTONS + 44.0)
        .show_separator_line(false)
        .frame(frame)
        .show(ui, contents)
        .inner
}

/// A fixed column beside the canvas.
pub fn side_panel<R>(panel: egui::Panel, width: f32, ui: &mut Ui, contents: impl FnOnce(&mut Ui) -> R) -> R {
    side_panel_with_gutter(panel, width, 0, ui, contents)
}

/// A fixed column with `gutter` points of its right margin given to its contents.
pub fn side_panel_with_gutter<R>(
    panel: egui::Panel,
    width: f32,
    gutter: i8,
    ui: &mut Ui,
    contents: impl FnOnce(&mut Ui) -> R,
) -> R {
    panel
        .exact_size(width)
        .resizable(false)
        .show_separator_line(false)
        .frame(shell().inner_margin(egui::Margin {
            left: PAGE,
            right: PAGE - gutter,
            top: 2,
            bottom: PAGE,
        }))
        .show(ui, contents)
        .inner
}

/// A page title.
pub fn title(ui: &mut Ui, text: &str) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 36.0), Sense::hover());
    ui.painter()
        .text(rect.left_center(), Align2::LEFT_CENTER, text, medium(26.0), INK);
}

/// A short message floating at the bottom of the window.
pub fn toast(ctx: &egui::Context, text: &str) {
    egui::Area::new(egui::Id::unique("toast"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -24.0))
        .interactable(false)
        .show(ctx, |ui| {
            let galley = ui.fonts_mut(|f| f.layout(text.to_owned(), regular(TEXT), WHITE, 560.0));
            let size = galley.size() + vec2(28.0, 18.0);
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            ui.painter().add(squircle(rect, RADIUS * 1.25, INK));
            ui.painter().galley(rect.min + vec2(14.0, 9.0), galley, WHITE);
        });
}

/// Text on one line, cut off with an ellipsis if it's wider than `width`.
fn one_line(ui: &Ui, text: &str, font: FontId, color: Color32, width: f32) -> Arc<egui::Galley> {
    let mut job = LayoutJob::simple(text.to_owned(), font, color, width);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.wrap.overflow_character = Some('…');
    ui.fonts_mut(|f| f.layout_job(job))
}

/// Muted text in a row, no wider than `width`.
pub fn muted_line(ui: &mut Ui, text: &str, width: f32) {
    let galley = one_line(ui, text, regular(TEXT), MUTED, width);
    let (rect, _) = ui.allocate_exact_size(vec2(galley.size().x, ROW), Sense::hover());
    let top = rect.center().y - galley.size().y / 2.0;
    ui.painter().galley(pos2(rect.left(), top), galley, MUTED);
}

/// A quiet label above a group of controls.
pub fn heading(ui: &mut Ui, text: &str) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::hover());
    let galley = one_line(ui, text, medium(HEADING), MUTED, rect.width());
    let top = rect.center().y - galley.size().y / 2.0;
    ui.painter().galley(pos2(rect.left(), top), galley, MUTED);
}

/// Air between groups.
pub fn gap(ui: &mut Ui) {
    ui.add_space(8.0);
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
    // Leave room for what a caller draws at the end of the row.
    let galley = one_line(ui, text, font, color, rect.width() - 36.0);
    let at = pos2(rect.left() + 8.0, rect.center().y - galley.size().y / 2.0);
    ui.painter().galley(at, galley, color);
    response
}

/// A layer row: returns the row itself, and whether its visibility dot was clicked.
pub fn layer_row(ui: &mut Ui, text: &str, on: bool, selected: bool) -> (Response, bool) {
    let response = row(ui, text, selected, if on { INK } else { MUTED });
    let dot = response.rect.right_center() - vec2(13.0, 0.0);
    let hit = Rect::from_center_size(dot, Vec2::splat(ROW));
    let clicked = ui
        .interact(hit, response.id.with("dot"), Sense::click())
        .clicked();
    if on {
        ui.painter().circle_filled(dot, 3.0, INK);
    } else {
        ui.painter().circle_stroke(dot, 2.5, Stroke::new(1.0, MUTED));
    }
    (response, clicked)
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
    toggle_button(ui, text, false)
}

/// A small text button that stays lit while its mode is on.
pub fn toggle_button(ui: &mut Ui, text: &str, on: bool) -> Response {
    let size = vec2(width_of(ui, text, medium(TEXT)) + 20.0, ROW);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    pill(ui, rect, text, on)
}

/// The one black button on a screen.
pub fn primary_button(ui: &mut Ui, text: &str) -> Response {
    let size = vec2(width_of(ui, text, medium(TEXT)) + 28.0, ROW);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let response = ui.interact(rect, ui.scope_id().with(text), Sense::click());
    let t = ui
        .ctx()
        .animate_bool_with_time(response.id, response.hovered(), 0.1);
    ui.painter().add(squircle(
        rect,
        RADIUS,
        INK.lerp_to_gamma(Color32::from_gray(60), t),
    ));
    ui.painter()
        .text(rect.center(), Align2::CENTER_CENTER, text, medium(TEXT), WHITE);
    response
}

/// One-of-many tabs, centred on `center`. Returns the index that was clicked.
pub fn tabs(ui: &mut Ui, center: Pos2, labels: &[&str], selected: usize) -> Option<usize> {
    let widths: Vec<f32> = labels
        .iter()
        .map(|l| width_of(ui, l, medium(TEXT)) + 24.0)
        .collect();
    let total = widths.iter().sum::<f32>() + GAP * (labels.len() - 1) as f32;
    let mut x = center.x - total / 2.0;
    let tray = Rect::from_center_size(center, vec2(total, ROW)).expand(TRAY_EDGE);
    ui.painter().add(squircle(tray, RADIUS * 1.25, WHITE));
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

/// What happened on a card: `open` is the card itself, `menu` is where to show its menu if
/// the menu button was just clicked, and `title` is where its name sits.
pub struct CardResponse {
    pub open: Response,
    pub menu: Option<Pos2>,
    pub title: Rect,
}

/// A project card: an image on a quiet tile, with a name below.
pub fn card(
    ui: &mut Ui,
    size: Vec2,
    image: Option<(egui::TextureId, Vec2)>,
    name: &str,
    menu_open: bool,
) -> CardResponse {
    const MENU_BUTTON: f32 = 26.0;
    let (rect, open) = ui.allocate_exact_size(size + vec2(0.0, 46.0), Sense::click());
    let tile = Rect::from_min_size(rect.min, size);
    let hovered = ui.rect_contains_pointer(rect);
    let t = ui.ctx().animate_bool_with_time(open.id, hovered, 0.1);
    ui.painter()
        .add(squircle(tile, RADIUS * 1.5, WHITE.lerp_to_gamma(WASH, t)));
    if let Some((id, image_size)) = image {
        let room = tile.shrink(14.0).size();
        let scale = (room.x / image_size.x).min(room.y / image_size.y);
        let at = Rect::from_center_size(tile.center(), image_size * scale);
        let uv = Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0));
        ui.painter().image(id, at, uv, Color32::WHITE);
    }
    let left = tile.left() + 2.0;
    let name = one_line(ui, name, medium(TEXT), INK, size.x - 4.0);
    ui.painter().galley(pos2(left, tile.bottom() + 8.0), name, INK);

    let mut menu = None;
    if hovered || menu_open {
        let top_right = tile.right_top() + vec2(-8.0 - MENU_BUTTON, 8.0);
        let button = Rect::from_min_size(top_right, Vec2::splat(MENU_BUTTON));
        let response = ui.interact(button, open.id.with("menu"), Sense::click());
        ui.painter().add(squircle(button, RADIUS * 0.75, WASH));
        for dx in [-5.0, 0.0, 5.0] {
            ui.painter()
                .circle_filled(button.center() + vec2(dx, 0.0), 1.5, INK);
        }
        if response.clicked() {
            menu = Some(button.right_bottom() + vec2(-150.0, 4.0));
        }
    }
    let title = Rect::from_min_size(pos2(tile.left(), tile.bottom() + 5.0), vec2(size.x, 22.0));
    CardResponse { open, menu, title }
}

pub enum MenuChoice {
    Waiting,
    Chosen(usize),
    Dismissed,
}

/// A small floating menu.
pub fn menu(ctx: &egui::Context, at: Pos2, items: &[&str]) -> MenuChoice {
    let size = vec2(150.0, ROW * items.len() as f32 + 8.0);
    let (rect, chosen) = egui::Area::new(egui::Id::unique("card menu"))
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            ui.painter().add(squircle(rect, RADIUS * 1.25, WHITE));
            ui.painter()
                .add(squircle_outline(rect, RADIUS * 1.25, Stroke::new(1.0, RAIL)));
            let mut chosen = None;
            for (i, item) in items.iter().enumerate() {
                let top = rect.min + vec2(4.0, 4.0 + ROW * i as f32);
                let row = Rect::from_min_size(top, vec2(size.x - 8.0, ROW));
                let response = ui.interact(row, ui.scope_id().with(item), Sense::click());
                ui.painter()
                    .add(squircle(row, RADIUS * 0.75, hover_fill(ui, &response, false)));
                let text = row.left_center() + vec2(8.0, 0.0);
                ui.painter()
                    .text(text, Align2::LEFT_CENTER, item, regular(TEXT), INK);
                if response.clicked() {
                    chosen = Some(i);
                }
            }
            (rect, chosen)
        })
        .inner;
    match chosen {
        Some(i) => MenuChoice::Chosen(i),
        None if clicked_away(ctx, rect) => MenuChoice::Dismissed,
        None => MenuChoice::Waiting,
    }
}

fn clicked_away(ctx: &egui::Context, rect: Rect) -> bool {
    ctx.input(|i| i.pointer.any_click() && i.pointer.interact_pos().is_some_and(|p| !rect.contains(p)))
}

pub enum NameChoice {
    Waiting,
    Save,
    Cancel,
}

/// Edit a name in place over `title`, in the same type, opening with the old name selected
/// so typing replaces it. Return or clicking away saves, Esc cancels.
pub fn name_field(ctx: &egui::Context, title: Rect, text: &mut String, opening: bool) -> NameChoice {
    let (rect, response) = egui::Area::new(egui::Id::unique("name field"))
        .order(egui::Order::Foreground)
        .fixed_pos(title.min)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(title.size(), Sense::hover());
            ui.painter().add(squircle(rect, RADIUS * 0.75, WHITE));
            ui.painter()
                .add(squircle_outline(rect, RADIUS * 0.75, Stroke::new(1.0, RAIL)));
            let inner = rect.shrink2(vec2(6.0, 1.0));
            let field = egui::TextEdit::singleline(text)
                .frame(egui::Frame::NONE)
                .margin(egui::Margin::ZERO)
                .font(medium(TEXT))
                .align(Align2::LEFT_CENTER)
                .min_size(inner.size())
                .desired_width(inner.width());
            let response = ui.put(inner, field);
            response.request_focus();
            if opening && let Some(mut state) = egui::TextEdit::load_state(ctx, response.id) {
                let all = egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(text.chars().count()),
                );
                state.cursor.set_char_range(Some(all));
                state.store(ctx, response.id);
            }
            (rect, response)
        })
        .inner;
    let (enter, escape) = ctx.input(|i| (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape)));
    if escape {
        NameChoice::Cancel
    } else if opening {
        // The click that chose Rename is still in the air, and it landed outside this field.
        NameChoice::Waiting
    } else if enter || response.lost_focus() || clicked_away(ctx, rect) {
        NameChoice::Save
    } else {
        NameChoice::Waiting
    }
}

/// The card that starts something new.
pub fn new_card(ui: &mut Ui, size: Vec2, name: &str, note: &str) -> Response {
    let (rect, response) = ui.allocate_exact_size(size + vec2(0.0, 46.0), Sense::click());
    let tile = Rect::from_min_size(rect.min, size);
    let t = ui
        .ctx()
        .animate_bool_with_time(response.id, response.hovered(), 0.1);
    ui.painter().add(squircle_outline(
        tile.shrink(0.5),
        RADIUS * 1.5,
        Stroke::new(1.0, RAIL.lerp_to_gamma(INK, t)),
    ));
    ui.painter().text(
        tile.center(),
        Align2::CENTER_CENTER,
        "+",
        regular(28.0),
        RAIL.lerp_to_gamma(INK, t.max(0.5)),
    );
    let left = tile.left() + 2.0;
    ui.painter().text(
        pos2(left, tile.bottom() + 16.0),
        Align2::LEFT_CENTER,
        name,
        medium(TEXT),
        INK,
    );
    ui.painter().text(
        pos2(left, tile.bottom() + 34.0),
        Align2::LEFT_CENTER,
        note,
        regular(HEADING),
        MUTED,
    );
    response
}

/// Fills the window while a file is being dragged over it.
pub fn drop_veil(ctx: &egui::Context, text: &str) {
    egui::Area::new(egui::Id::unique("drop veil"))
        .order(egui::Order::Foreground)
        .fixed_pos(Pos2::ZERO)
        .interactable(false)
        .show(ctx, |ui| {
            let window = ctx.content_rect();
            ui.painter()
                .rect_filled(window, 0.0, Color32::from_white_alpha(235));
            ui.painter().add(squircle_outline(
                window.shrink(16.0),
                RADIUS * 2.0,
                Stroke::new(1.5, INK),
            ));
            ui.painter()
                .text(window.center(), Align2::CENTER_CENTER, text, medium(16.0), INK);
        });
}

/// A labelled slider: name and value above a hairline rail with a black knob.
pub fn slider(ui: &mut Ui, label: &str, value: &mut f32, range: RangeInclusive<f32>) -> bool {
    labelled_slider(ui, label, value, range, 2)
}

/// A slider for a whole number.
pub fn whole_slider(ui: &mut Ui, label: &str, value: &mut f32, range: RangeInclusive<f32>) -> bool {
    labelled_slider(ui, label, value, range, 0)
}

fn labelled_slider(
    ui: &mut Ui,
    label: &str,
    value: &mut f32,
    range: RangeInclusive<f32>,
    decimals: usize,
) -> bool {
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
        format!("{value:.decimals$}"),
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
