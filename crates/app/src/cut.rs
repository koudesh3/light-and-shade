//! The cutting board: trace pieces out of a painting.

use crate::canvas::{self, Placement};
use crate::design;
use crate::studio::Nav;
use anyhow::{Context, Result};
use eframe::egui::{
    self, Color32, ColorImage, Key, Pos2, Rect, Sense, Shape, Stroke, TextureHandle, TextureOptions, pos2,
};
use image::imageops::{self, FilterType};
use las_core::{Cut, Project, View};

/// How close, in screen points, the pointer must be to count as touching a point.
const REACH: f32 = 10.0;
/// How far the pointer moves between freehand points, in screen points.
const STROKE_STEP: f32 = 6.0;
const HISTORY: usize = 200;

/// What dragging on the canvas does.
#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Cut,
    Hole,
    Move,
}

/// Which outline a grabbed point belongs to.
#[derive(Clone, Copy)]
enum Outline {
    Draft,
    Piece(usize),
    Hole(usize, usize),
}

#[derive(Clone)]
struct Snapshot {
    cuts: Vec<Cut>,
    draft: Vec<[f32; 2]>,
}

pub struct CutBoard {
    project: Project,
    texture: TextureHandle,
    view: View,
    tool: Tool,
    draft: Vec<[f32; 2]>,
    selected: Option<usize>,
    /// A point being dragged, and its index in its outline.
    grabbed: Option<(Outline, usize)>,
    /// The last freehand point on screen, while a stroke is under way.
    stroke: Option<Pos2>,
    history: Vec<Snapshot>,
    /// Snapshots stepped back from, most recent last. Any new change discards them.
    undone: Vec<Snapshot>,
    /// Cuts changed since this board opened, so the layers need rebuilding.
    dirty: bool,
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
        // Cuts can outnumber layers if the app closed before they were rebuilt.
        let dirty = project.cuts.len() != project.layers.len();
        Ok(Self {
            texture: load_texture(ctx, &project)?,
            project,
            view: View::default(),
            tool: Tool::Cut,
            draft: Vec::new(),
            selected: None,
            grabbed: None,
            stroke: None,
            history: Vec::new(),
            undone: Vec::new(),
            dirty,
            error: None,
        })
    }

    fn painting(&self) -> [f32; 2] {
        [self.project.width as f32, self.project.height as f32]
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            cuts: self.project.cuts.clone(),
            draft: self.draft.clone(),
        }
    }

    fn remember(&mut self) {
        self.undone.clear();
        self.history.push(self.snapshot());
        if self.history.len() > HISTORY {
            self.history.remove(0);
        }
    }

    fn restore(&mut self, snapshot: Snapshot) {
        self.project.cuts = snapshot.cuts;
        self.draft = snapshot.draft;
        self.selected = None;
        self.grabbed = None;
        self.save();
    }

    fn undo(&mut self) {
        if let Some(snapshot) = self.history.pop() {
            self.undone.push(self.snapshot());
            self.restore(snapshot);
        }
    }

    fn redo(&mut self) {
        if let Some(snapshot) = self.undone.pop() {
            self.history.push(self.snapshot());
            self.restore(snapshot);
        }
    }

    fn save(&mut self) {
        self.dirty = true;
        self.error = self.project.save().err().map(|e| format!("{e:#}"));
    }

    /// Turn the draft into a piece, if it has enough points to enclose anything.
    fn close_draft(&mut self) {
        if self.draft.len() < 3 {
            return;
        }
        if self.tool == Tool::Hole {
            // Punch the selected piece, or else the frontmost one the hole starts on.
            let start = self.draft[0];
            let target = self
                .selected
                .or_else(|| self.project.cuts.iter().rposition(|c| c.contains(start)));
            let Some(target) = target else {
                self.error = Some("Start a hole inside a piece.".into());
                return;
            };
            self.remember();
            let hole = std::mem::take(&mut self.draft);
            self.project.cuts[target].holes.push(hole);
            self.selected = Some(target);
            self.save();
            return;
        }
        self.remember();
        let number = self.project.cuts.len() + 1;
        let points = std::mem::take(&mut self.draft);
        self.project
            .cuts
            .push(Cut::new(format!("Piece {number}"), points));
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
        let (cut, hole, take_move, enter, escape, back, (undo, redo)) = ctx.input(|i| {
            (
                i.key_pressed(Key::C),
                i.key_pressed(Key::H),
                i.key_pressed(Key::M),
                i.key_pressed(Key::Enter),
                i.key_pressed(Key::Escape),
                i.key_pressed(Key::Backspace) || i.key_pressed(Key::Delete),
                canvas::history_keys(i),
            )
        });
        if cut {
            self.tool = Tool::Cut;
        }
        if hole {
            self.tool = Tool::Hole;
        }
        if take_move {
            self.tool = Tool::Move;
        }
        if enter {
            self.close_draft();
        }
        if escape && !self.draft.is_empty() {
            self.remember();
            self.draft.clear();
            self.error = None;
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
        if redo {
            self.redo();
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) -> Option<Nav> {
        let mut nav = None;
        const TOOLS: [Tool; 3] = [Tool::Cut, Tool::Hole, Tool::Move];
        let selected = TOOLS.iter().position(|t| *t == self.tool).unwrap_or(0);
        if let Some(i) = design::tabs(ui, ui.max_rect().center(), &["Cut", "Hole", "Move"], selected) {
            self.tool = TOOLS[i];
        }
        ui.horizontal_centered(|ui| {
            design::card_tray(ui, design::SIDEBAR_CARD, 1, |ui, cell| {
                if design::cell_button(ui, "‹ Paintings", false, cell).clicked() {
                    self.close_draft();
                    nav = Some(if self.dirty {
                        Nav::BakeHome(self.project.clone())
                    } else {
                        Nav::Home
                    });
                }
            });
            ui.add_space(4.0);
            design::muted_line(ui, &self.project.name, 260.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                design::tray(ui, |ui| {
                    let finished = !self.project.cuts.is_empty() || self.draft.len() >= 3;
                    if finished && design::primary_button(ui, "Done").clicked() {
                        self.close_draft();
                        nav = Some(Nav::Bake(self.project.clone()));
                    }
                });
            });
        });
        nav
    }

    fn left_panel(&mut self, ui: &mut egui::Ui) {
        design::group(ui, |ui| {
            ui.set_min_height(ui.available_height());
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
            ui.horizontal_wrapped(|ui| {
                if !self.history.is_empty() && design::button(ui, "Undo").clicked() {
                    self.undo();
                }
                if !self.undone.is_empty() && design::button(ui, "Redo").clicked() {
                    self.redo();
                }
                if self.selected.is_some() && design::button(ui, "Delete").clicked() {
                    self.remove_selected();
                }
                let holes = self
                    .selected
                    .is_some_and(|i| !self.project.cuts[i].holes.is_empty());
                if holes && design::button(ui, "Fill holes").clicked() {
                    self.remember();
                    if let Some(i) = self.selected {
                        self.project.cuts[i].holes.clear();
                    }
                    self.save();
                }
            });
        });
    }

    fn pointer(&mut self, ui: &egui::Ui, inside: bool, panning: bool, rect: Rect) {
        let painting = self.painting();
        let placed = Placement::of(ui, &self.view, rect, painting);
        let to_image = |p: Pos2| {
            let [x, y] = placed.to_painting(p);
            [x.clamp(0.0, painting[0]), y.clamp(0.0, painting[1])]
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
        let touching = |p: [f32; 2]| placed.to_screen(p).distance(at) <= REACH;

        if pressed && inside && !panning {
            let mut outlines: Vec<(Outline, &Vec<[f32; 2]>)> = Vec::new();
            if let Some(i) = self.selected {
                outlines.push((Outline::Piece(i), &self.project.cuts[i].points));
                for (h, hole) in self.project.cuts[i].holes.iter().enumerate() {
                    outlines.push((Outline::Hole(i, h), hole));
                }
            }
            outlines.push((Outline::Draft, &self.draft));
            let hit = outlines.into_iter().find_map(|(outline, points)| {
                // The draft's first point closes the shape, so it can't be grabbed.
                let start = usize::from(matches!(outline, Outline::Draft));
                let index = points.iter().skip(start).position(|p| touching(*p))?;
                Some((outline, index + start))
            });
            if let Some(hit) = hit {
                self.remember();
                self.grabbed = Some(hit);
            } else if self.draft.len() >= 3 && touching(self.draft[0]) {
                self.close_draft();
            } else {
                self.remember();
                self.draft.push(to_image(at));
                if self.tool != Tool::Hole {
                    self.selected = None;
                }
                self.stroke = Some(at);
            }
        } else if down {
            if let Some((outline, i)) = self.grabbed {
                let moved = to_image(at);
                match outline {
                    Outline::Draft => self.draft[i] = moved,
                    Outline::Piece(c) => self.project.cuts[c].points[i] = moved,
                    Outline::Hole(c, h) => self.project.cuts[c].holes[h][i] = moved,
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
        let (space, other_button, primary) = ui.input(|i| {
            (
                i.key_down(Key::Space),
                i.pointer.middle_down() || i.pointer.secondary_down(),
                i.pointer.primary_down(),
            )
        });
        let panning = self.tool == Tool::Move || space || other_button;
        if response.dragged() && panning {
            let ppp = ui.pixels_per_point();
            self.view.pan[0] += response.drag_delta().x * ppp;
            self.view.pan[1] += response.drag_delta().y * ppp;
        }
        if let Some(p) = response.hover_pos() {
            canvas::navigate(ui, &mut self.view, p, rect, painting);
            let icon = match (panning, primary || other_button) {
                (true, true) => egui::CursorIcon::Grabbing,
                (true, false) => egui::CursorIcon::Grab,
                (false, _) => egui::CursorIcon::Crosshair,
            };
            ui.ctx().set_cursor_icon(icon);
        }
        let badge = design::zoom_badge_rect(ui, rect, self.view.zoom);
        let over_badge = ui
            .input(|i| i.pointer.interact_pos())
            .is_some_and(|p| badge.contains(p));
        self.pointer(ui, response.contains_pointer() && !over_badge, panning, rect);

        let placed = Placement::of(ui, &self.view, rect, painting);
        let to_screen = |p: [f32; 2]| placed.to_screen(p);
        design::canvas_surface(ui, rect);
        let painter = ui.painter_at(rect);
        let image = Rect::from_min_size(to_screen([0.0, 0.0]), egui::Vec2::from(painting) * placed.scale());
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
            for hole in &cut.holes {
                let points: Vec<Pos2> = hole.iter().map(|p| to_screen(*p)).collect();
                line(points.clone(), true, if chosen { 2.0 } else { 1.25 });
                if chosen {
                    points.iter().for_each(|p| dot(*p, 3.5));
                }
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
        design::canvas_corners(ui, rect);
        if design::zoom_badge(ui, badge, self.view.zoom).clicked() {
            self.view = View::default();
        }
        let low_resolution = format!(
            "This image is only {} × {}, so it will look soft. Dragging from Photos can give a small \
             preview: export the photo, then drop that file instead.",
            self.project.width, self.project.height
        );
        let hint = if !self.draft.is_empty() {
            Some("Click the first point or press Return to close. Esc to cancel.")
        } else if self.tool == Tool::Hole {
            Some("Trace inside a piece to punch out that area.")
        } else if self.project.cuts.is_empty() && self.project.is_low_resolution() {
            Some(low_resolution.as_str())
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
        design::side_panel(egui::Panel::left("pieces"), design::SIDEBAR, ui, |ui| {
            self.left_panel(ui)
        });
        egui::CentralPanel::default()
            .frame(design::canvas_frame(false))
            .show(ui, |ui| self.canvas(ui));
        nav
    }
}
