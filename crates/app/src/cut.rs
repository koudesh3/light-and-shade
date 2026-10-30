//! The cutting board: trace pieces out of a painting.

use crate::design;
use crate::studio::Nav;
use anyhow::{Context, Result};
use eframe::egui::{
    self, Color32, ColorImage, Key, Pos2, Rect, Sense, Shape, Stroke, TextureHandle, TextureOptions, pos2,
    vec2,
};
use image::imageops::{self, FilterType};
use las_core::{Cut, Project, View};

/// How close, in screen points, the pointer must be to count as touching a point.
const REACH: f32 = 10.0;
/// How far the pointer moves between freehand points, in screen points.
const STROKE_STEP: f32 = 6.0;
const HISTORY: usize = 200;

#[derive(Clone)]
struct Snapshot {
    cuts: Vec<Cut>,
    draft: Vec<[f32; 2]>,
}

pub struct CutBoard {
    project: Project,
    texture: TextureHandle,
    view: View,
    draft: Vec<[f32; 2]>,
    selected: Option<usize>,
    /// A point being dragged: the cut it's in (`None` for the draft) and its index.
    grabbed: Option<(Option<usize>, usize)>,
    /// The last freehand point on screen, while a stroke is under way.
    stroke: Option<Pos2>,
    history: Vec<Snapshot>,
    error: Option<String>,
}

fn load_texture(ctx: &egui::Context, project: &Project) -> Result<TextureHandle> {
    let source = project
        .source
        .as_ref()
        .context("this painting has no source image")?;
    let mut image = image::open(project.dir.join(source))
        .with_context(|| format!("reading {}", source.display()))?
        .into_rgba8();
    let limit = ctx.input(|i| i.max_texture_side).min(4096) as u32;
    let longest = image.width().max(image.height());
    if longest > limit {
        let scale = limit as f32 / longest as f32;
        let (w, h) = (image.width() as f32 * scale, image.height() as f32 * scale);
        image = imageops::resize(&image, w as u32, h as u32, FilterType::Triangle);
    }
    let size = [image.width() as usize, image.height() as usize];
    let pixels = ColorImage::from_rgba_unmultiplied(size, image.as_raw());
    Ok(ctx.load_texture("source", pixels, TextureOptions::LINEAR))
}

impl CutBoard {
    pub fn new(ctx: &egui::Context, project: Project) -> Result<Self> {
        Ok(Self {
            texture: load_texture(ctx, &project)?,
            project,
            view: View::default(),
            draft: Vec::new(),
            selected: None,
            grabbed: None,
            stroke: None,
            history: Vec::new(),
            error: None,
        })
    }

    fn painting(&self) -> [f32; 2] {
        [self.project.width as f32, self.project.height as f32]
    }

    fn remember(&mut self) {
        self.history.push(Snapshot {
            cuts: self.project.cuts.clone(),
            draft: self.draft.clone(),
        });
        if self.history.len() > HISTORY {
            self.history.remove(0);
        }
    }

    fn undo(&mut self) {
        if let Some(snapshot) = self.history.pop() {
            self.project.cuts = snapshot.cuts;
            self.draft = snapshot.draft;
            self.selected = None;
            self.grabbed = None;
            self.save();
        }
    }

    fn save(&mut self) {
        self.error = self.project.save().err().map(|e| format!("{e:#}"));
    }

    /// Turn the draft into a piece, if it has enough points to enclose anything.
    fn close_draft(&mut self) {
        if self.draft.len() < 3 {
            return;
        }
        self.remember();
        let number = self.project.cuts.len() + 1;
        let points = std::mem::take(&mut self.draft);
        self.project.cuts.push(Cut {
            name: format!("Piece {number}"),
            points,
        });
        self.selected = None;
        self.save();
    }

    fn remove_selected(&mut self) {
        if let Some(i) = self.selected.take() {
            self.remember();
            self.project.cuts.remove(i);
            self.save();
        }
    }

    fn keys(&mut self, ctx: &egui::Context) {
        let (enter, escape, back, undo) = ctx.input(|i| {
            (
                i.key_pressed(Key::Enter),
                i.key_pressed(Key::Escape),
                i.key_pressed(Key::Backspace) || i.key_pressed(Key::Delete),
                i.modifiers.command && i.key_pressed(Key::Z),
            )
        });
        if enter {
            self.close_draft();
        }
        if escape && !self.draft.is_empty() {
            self.remember();
            self.draft.clear();
        }
        if back {
            if self.draft.is_empty() {
                self.remove_selected();
            } else {
                self.remember();
                self.draft.pop();
            }
        }
        if undo {
            self.undo();
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) -> Option<Nav> {
        let mut nav = None;
        let rect = ui.max_rect();
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            &self.project.name,
            egui::FontId::proportional(13.0),
            design::INK,
        );
        ui.horizontal_centered(|ui| {
            ui.add_space(design::TRAFFIC_LIGHTS);
            if design::button(ui, "‹ Paintings").clicked() {
                nav = Some(Nav::Home);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let finished = !self.project.cuts.is_empty() || self.draft.len() >= 3;
                if finished && design::primary_button(ui, "Done").clicked() {
                    self.close_draft();
                    nav = Some(Nav::Bake(self.project.clone()));
                }
                if design::button(ui, &format!("{:.0}%", self.view.zoom * 100.0)).clicked() {
                    self.view = View::default();
                }
            });
        });
        nav
    }

    fn left_panel(&mut self, ui: &mut egui::Ui) {
        design::heading(ui, "Pieces");
        if self.project.cuts.is_empty() {
            ui.colored_label(design::MUTED, "Nothing cut yet");
        }
        let mut chosen = None;
        egui::ScrollArea::vertical()
            .max_height(ui.available_height() - 70.0)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for (i, cut) in self.project.cuts.iter().enumerate().rev() {
                    if design::row(ui, &cut.name, self.selected == Some(i), design::INK).clicked() {
                        chosen = Some(i);
                    }
                }
            });
        if let Some(i) = chosen {
            self.selected = (self.selected != Some(i)).then_some(i);
        }
        design::gap(ui);
        ui.horizontal(|ui| {
            if !self.history.is_empty() && design::button(ui, "Undo").clicked() {
                self.undo();
            }
            if self.selected.is_some() && design::button(ui, "Delete").clicked() {
                self.remove_selected();
            }
        });
    }

    fn pointer(&mut self, ui: &egui::Ui, inside: bool, panning: bool, rect: Rect) {
        let painting = self.painting();
        let target = [rect.width(), rect.height()];
        let (scale, origin) = (
            self.view.scale(target, painting),
            self.view.origin(target, painting),
        );
        let to_screen = |p: [f32; 2]| rect.min + vec2(origin[0] + p[0] * scale, origin[1] + p[1] * scale);
        let to_image = |p: Pos2| {
            let v = p - rect.min;
            [
                ((v.x - origin[0]) / scale).clamp(0.0, painting[0]),
                ((v.y - origin[1]) / scale).clamp(0.0, painting[1]),
            ]
        };
        let (pressed, down, released, at) = ui.input(|i| {
            let p = &i.pointer;
            (
                p.primary_pressed(),
                p.primary_down(),
                p.primary_released(),
                p.interact_pos(),
            )
        });
        let Some(at) = at else { return };
        let touching = |p: [f32; 2]| to_screen(p).distance(at) <= REACH;

        if pressed && inside && !panning {
            let selected_points = self.selected.map(|i| (Some(i), &self.project.cuts[i].points));
            let draft_points = (None, &self.draft);
            let hit = selected_points
                .into_iter()
                .chain([draft_points])
                .find_map(|(cut, pts)| {
                    // The draft's first point closes the shape, so it can't be grabbed.
                    let start = if cut.is_none() { 1 } else { 0 };
                    pts.iter()
                        .skip(start)
                        .position(|p| touching(*p))
                        .map(|i| (cut, i + start))
                });
            if let Some(hit) = hit {
                self.remember();
                self.grabbed = Some(hit);
            } else if self.draft.len() >= 3 && touching(self.draft[0]) {
                self.close_draft();
            } else {
                self.remember();
                self.draft.push(to_image(at));
                self.selected = None;
                self.stroke = Some(at);
            }
        } else if down {
            if let Some((cut, i)) = self.grabbed {
                let moved = to_image(at);
                match cut {
                    Some(c) => self.project.cuts[c].points[i] = moved,
                    None => self.draft[i] = moved,
                }
            } else if let Some(last) = self.stroke
                && last.distance(at) >= STROKE_STEP
            {
                self.draft.push(to_image(at));
                self.stroke = Some(at);
            }
        }
        if released {
            if self.grabbed.take().is_some() {
                self.save();
            }
            // A stroke that ends back at its start encloses a piece.
            if self.stroke.take().is_some() && self.draft.len() > 3 && touching(self.draft[0]) {
                self.close_draft();
            }
        }
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let rect = ui.max_rect();
        let response = ui.allocate_rect(rect, Sense::click_and_drag());
        let painting = self.painting();
        let target = [rect.width(), rect.height()];
        let panning =
            ui.input(|i| i.key_down(Key::Space) || i.pointer.middle_down() || i.pointer.secondary_down());
        if response.dragged() && panning {
            self.view.pan[0] += response.drag_delta().x;
            self.view.pan[1] += response.drag_delta().y;
        }
        if let Some(p) = response.hover_pos() {
            let (scroll, pinch) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let factor = pinch * (scroll * 0.002).exp();
            if factor != 1.0 {
                let at = p - rect.min;
                self.view.zoom_at(factor, [at.x, at.y], target, painting);
            }
        }
        self.pointer(ui, response.contains_pointer(), panning, rect);

        let (scale, origin) = (
            self.view.scale(target, painting),
            self.view.origin(target, painting),
        );
        let to_screen = |p: [f32; 2]| rect.min + vec2(origin[0] + p[0] * scale, origin[1] + p[1] * scale);
        let painter = ui.painter_at(rect);
        let image = Rect::from_min_size(to_screen([0.0, 0.0]), egui::Vec2::from(painting) * scale);
        let uv = Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0));
        painter.image(self.texture.id(), image, uv, Color32::WHITE);

        let line = |points: Vec<Pos2>, closed: bool, width: f32| {
            let shape = |w, color| {
                let stroke = Stroke::new(w, color);
                if closed {
                    Shape::closed_line(points.clone(), stroke)
                } else {
                    Shape::line(points.clone(), stroke)
                }
            };
            painter.add(shape(width + 2.0, Color32::WHITE));
            painter.add(shape(width, design::INK));
        };
        let dot = |p: Pos2, r: f32| {
            painter.circle(p, r, Color32::WHITE, Stroke::new(1.5, design::INK));
        };
        for (i, cut) in self.project.cuts.iter().enumerate() {
            let points: Vec<Pos2> = cut.points.iter().map(|p| to_screen(*p)).collect();
            let chosen = self.selected == Some(i);
            line(points.clone(), true, if chosen { 2.0 } else { 1.25 });
            if chosen {
                points.iter().for_each(|p| dot(*p, 3.5));
            }
        }
        if !self.draft.is_empty() {
            let mut points: Vec<Pos2> = self.draft.iter().map(|p| to_screen(*p)).collect();
            if let Some(hover) = response.hover_pos().filter(|_| self.stroke.is_none()) {
                points.push(hover);
                ui.ctx().request_repaint();
            }
            line(points, false, 1.5);
            dot(to_screen(self.draft[0]), 5.0);
        }
        let hint = if !self.draft.is_empty() {
            Some("Click the first point or press Return to close. Esc to cancel.")
        } else if self.project.cuts.is_empty() {
            Some("Click around a piece, or drag to draw it.")
        } else {
            None
        };
        if let Some(hint) = hint.filter(|_| self.error.is_none()) {
            design::toast(ui.ctx(), hint);
        }
        if let Some(error) = &self.error {
            design::toast(ui.ctx(), error);
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<Nav> {
        self.keys(&ui.ctx().clone());
        let nav = design::top_bar(ui, |ui| self.toolbar(ui));
        design::side_panel(egui::Panel::left("pieces"), 216.0, ui, |ui| self.left_panel(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(design::WHITE))
            .show(ui, |ui| self.canvas(ui));
        nav
    }
}
