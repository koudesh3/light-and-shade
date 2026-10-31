//! Painting normals onto a piece, in the light room, under the real light.

use crate::ball::Sphere;
use crate::canvas::{self, Placement};
use crate::design;
use anyhow::Result;
use eframe::egui::{self, Color32, Key, Pos2, Rect, Shape, Stroke as Line, vec2};
use las_core::{NormalMap, Project, Stroke, StrokeKind, View, encode_normal};

const HISTORY: usize = 100;
const FACING_YOU: [f32; 3] = [0.0, 0.0, 1.0];
/// How close, in screen points, the pointer must be to count as touching a point.
const REACH: f32 = 10.0;
/// How far the pointer moves between freehand shape points, in screen points.
const STROKE_STEP: f32 = 6.0;

#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Brush,
    Shape,
    Erase,
}

pub struct Painter {
    normal: [f32; 3],
    /// How many directions the sphere offers, or `None` for all of them.
    shades: Option<usize>,
    tool: Tool,
    radius: f32,
    pub piece: usize,
    map: NormalMap,
    /// Points of the brush stroke under way, in source pixels.
    stroke: Option<Vec<[f32; 2]>>,
    /// The shape being placed, in source pixels. It stays open across clicks.
    shape: Vec<[f32; 2]>,
    /// While dragging a shape: the last freehand point on screen, and whether the drag began it.
    shape_drag: Option<(Pos2, bool)>,
    /// The finished shape still selected, by its index in the piece's strokes. Picking a
    /// direction changes it, until the next shape starts.
    active: Option<usize>,
    /// The active shape changed and the map on disk hasn't caught up.
    unsaved: bool,
    sphere: Sphere,
    /// Earlier stroke lists, and the piece each belongs to.
    history: Vec<(usize, Vec<Stroke>)>,
    /// Stroke lists stepped back from, most recent last. Any new change discards them.
    undone: Vec<(usize, Vec<Stroke>)>,
    /// Dots stepped back from while placing a shape.
    undone_dots: Vec<[f32; 2]>,
}

impl Painter {
    pub fn new(project: &Project) -> Self {
        let piece = project.cuts.len() - 1;
        Self {
            normal: FACING_YOU,
            shades: None,
            tool: Tool::Brush,
            radius: 60.0,
            piece,
            map: NormalMap::from_strokes(&project.layers[piece], &project.cuts[piece].strokes),
            stroke: None,
            shape: Vec::new(),
            shape_drag: None,
            active: None,
            unsaved: false,
            sphere: Sphere::default(),
            history: Vec::new(),
            undone: Vec::new(),
            undone_dots: Vec::new(),
        }
    }

    pub fn map(&self) -> &NormalMap {
        &self.map
    }

    pub fn select(&mut self, project: &Project, piece: usize) {
        self.piece = piece;
        self.map = NormalMap::from_strokes(&project.layers[piece], &project.cuts[piece].strokes);
        self.stroke = None;
        self.shape.clear();
        self.shape_drag = None;
        self.active = None;
    }

    /// Give the active shape the current direction, and save once the pointer lets go.
    fn sync_active(&mut self, ui: &egui::Ui, project: &mut Project) -> Result<bool> {
        let mut changed = false;
        if let Some(i) = self.active {
            let strokes = &mut project.cuts[self.piece].strokes;
            if strokes[i].normal != self.normal {
                strokes[i].normal = self.normal;
                self.map = NormalMap::from_strokes(&project.layers[self.piece], strokes);
                self.unsaved = true;
                changed = true;
            }
        }
        if self.unsaved && !ui.input(|i| i.pointer.primary_down()) {
            self.unsaved = false;
            project.store_normals(self.piece, &self.map)?;
        }
        Ok(changed)
    }

    /// Fill the shape being placed, if it has enough points to enclose anything.
    fn close_shape(&mut self, project: &mut Project) -> Result<bool> {
        self.shape_drag = None;
        if self.shape.len() < 3 {
            return Ok(false);
        }
        let stroke = Stroke {
            kind: StrokeKind::Shape,
            normal: self.normal,
            radius: 0.0,
            points: std::mem::take(&mut self.shape),
        };
        self.commit(project, stroke)?;
        self.active = Some(project.cuts[self.piece].strokes.len() - 1);
        Ok(true)
    }

    fn remember(&mut self, project: &Project) {
        self.undone.clear();
        self.history
            .push((self.piece, project.cuts[self.piece].strokes.clone()));
        if self.history.len() > HISTORY {
            self.history.remove(0);
        }
    }

    fn commit(&mut self, project: &mut Project, stroke: Stroke) -> Result<()> {
        self.remember(project);
        self.active = None;
        self.map.apply(&stroke);
        project.cuts[self.piece].strokes.push(stroke);
        project.store_normals(self.piece, &self.map)
    }

    fn undo(&mut self, project: &mut Project) -> Result<bool> {
        let Some((piece, strokes)) = self.history.pop() else {
            return Ok(false);
        };
        let current = std::mem::replace(&mut project.cuts[piece].strokes, strokes);
        self.undone.push((piece, current));
        self.select(project, piece);
        project.store_normals(piece, &self.map)?;
        Ok(true)
    }

    fn redo(&mut self, project: &mut Project) -> Result<bool> {
        let Some((piece, strokes)) = self.undone.pop() else {
            return Ok(false);
        };
        let current = std::mem::replace(&mut project.cuts[piece].strokes, strokes);
        self.history.push((piece, current));
        self.select(project, piece);
        project.store_normals(piece, &self.map)?;
        Ok(true)
    }

    fn clear(&mut self, project: &mut Project) -> Result<()> {
        self.remember(project);
        project.cuts[self.piece].strokes.clear();
        self.select(project, self.piece);
        project.store_normals(self.piece, &self.map)
    }

    /// The right-hand controls. Returns whether the map changed.
    pub fn panel(&mut self, ui: &mut egui::Ui, project: &mut Project) -> Result<bool> {
        let mut changed = false;
        design::group(ui, |ui| {
            design::heading(ui, "Direction");
            ui.add_space(4.0);
            ui.vertical_centered(|ui| self.sphere.show(ui, &mut self.normal, self.shades));
            ui.add_space(4.0);
            let row = vec2(ui.available_width(), design::ROW);
            ui.allocate_ui_with_layout(row, egui::Layout::left_to_right(egui::Align::Center), |ui| {
                let [x, y, z] = self.normal;
                ui.colored_label(design::MUTED, format!("{x:.2}  {y:.2}  {z:.2}"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if design::button(ui, "Facing you").clicked() {
                        self.normal = FACING_YOU;
                    }
                });
            });
            let (rect, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), design::ROW), egui::Sense::hover());
            const SHADES: [Option<usize>; 3] = [None, Some(32), Some(16)];
            let selected = SHADES.iter().position(|s| *s == self.shades).unwrap_or(0);
            if let Some(i) = design::tabs(ui, rect.center(), &["All", "32", "16"], selected) {
                self.shades = SHADES[i];
            }
            changed |= self.sync_active(ui, project)?;
            anyhow::Ok(())
        })?;
        design::gap(ui);
        design::group(ui, |ui| {
            design::heading(ui, "Tool");
            let (rect, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), design::ROW), egui::Sense::hover());
            const TOOLS: [Tool; 3] = [Tool::Brush, Tool::Shape, Tool::Erase];
            let selected = TOOLS.iter().position(|t| *t == self.tool).unwrap_or(0);
            if let Some(i) = design::tabs(ui, rect.center(), &["Brush", "Shape", "Erase"], selected) {
                self.tool = TOOLS[i];
                self.shape.clear();
                self.active = None;
            }
            ui.add_space(4.0);
            design::whole_slider(ui, "Size", &mut self.radius, 6.0..=300.0);
            ui.horizontal_wrapped(|ui| {
                if design::button(ui, "Fill piece").clicked() {
                    let fill = Stroke {
                        kind: StrokeKind::Fill,
                        normal: self.normal,
                        radius: 0.0,
                        points: Vec::new(),
                    };
                    self.commit(project, fill)?;
                    changed = true;
                }
                if !self.history.is_empty() && design::button(ui, "Undo").clicked() {
                    changed |= self.undo(project)?;
                }
                if !self.undone.is_empty() && design::button(ui, "Redo").clicked() {
                    changed |= self.redo(project)?;
                }
                if !project.cuts[self.piece].strokes.is_empty() && design::button(ui, "Clear").clicked() {
                    self.clear(project)?;
                    changed = true;
                }
                anyhow::Ok(())
            })
            .inner
        })?;
        Ok(changed)
    }

    pub fn keys(&mut self, ctx: &egui::Context, project: &mut Project) -> Result<bool> {
        if ctx.egui_wants_keyboard_input() {
            return Ok(false);
        }
        let (enter, escape, back, (undo, redo)) = ctx.input(|i| {
            for (key, tool) in [
                (Key::B, Tool::Brush),
                (Key::L, Tool::Shape),
                (Key::E, Tool::Erase),
            ] {
                if i.key_pressed(key) && self.tool != tool {
                    self.tool = tool;
                    self.shape.clear();
                    self.active = None;
                }
            }
            if i.key_pressed(Key::OpenBracket) {
                self.radius = (self.radius - 10.0).max(6.0);
            }
            if i.key_pressed(Key::CloseBracket) {
                self.radius = (self.radius + 10.0).min(300.0);
            }
            (
                i.key_pressed(Key::Enter),
                i.key_pressed(Key::Escape),
                i.key_pressed(Key::Backspace) || i.key_pressed(Key::Delete),
                canvas::history_keys(i),
            )
        });
        if escape {
            self.shape.clear();
            self.undone_dots.clear();
            self.active = None;
        }
        // While placing a shape, stepping back removes its last point.
        if (back || undo)
            && let Some(dot) = self.shape.pop()
        {
            self.undone_dots.push(dot);
            return Ok(false);
        }
        if redo {
            return match self.undone_dots.pop() {
                Some(dot) => {
                    self.shape.push(dot);
                    Ok(false)
                }
                None => self.redo(project),
            };
        }
        if enter {
            return self.close_shape(project);
        }
        if undo || (back && self.active.is_some()) {
            self.undo(project)
        } else {
            Ok(false)
        }
    }

    /// Handle the pointer over the canvas. Returns whether the map changed.
    pub fn pointer(
        &mut self,
        ui: &egui::Ui,
        project: &mut Project,
        view: &View,
        rect: Rect,
        inside: bool,
    ) -> Result<bool> {
        let painting = [project.width as f32, project.height as f32];
        let placed = Placement::of(ui, view, rect, painting);
        let (pressed, down, released, alt, at) = ui.input(|i| {
            let p = &i.pointer;
            (
                p.primary_pressed(),
                p.primary_down(),
                p.primary_released(),
                i.modifiers.alt,
                p.interact_pos(),
            )
        });
        let Some(at) = at else { return Ok(false) };
        let point = placed.to_painting(at);
        let color = encode_normal(if self.tool == Tool::Erase {
            FACING_YOU
        } else {
            self.normal
        });
        let placing_shape = self.tool == Tool::Shape;
        let on_first = self
            .shape
            .first()
            .is_some_and(|p| placed.to_screen(*p).distance(at) <= REACH);
        let mut changed = false;

        if pressed && inside {
            if alt {
                if let Some(normal) = self.map.sample(point) {
                    self.normal = normal;
                }
                return Ok(false);
            }
            if placing_shape {
                if self.shape.len() >= 3 && on_first {
                    return self.close_shape(project);
                }
                self.active = None;
                self.undone_dots.clear();
                self.shape_drag = Some((at, self.shape.is_empty()));
                self.shape.push(point);
                return Ok(false);
            }
            self.stroke = Some(vec![point]);
            self.map.segment(point, point, self.radius, color);
            changed = true;
        } else if down && let Some((last, began)) = self.shape_drag {
            if last.distance(at) >= STROKE_STEP {
                self.shape.push(point);
                self.shape_drag = Some((at, began));
            }
        } else if down && let Some(points) = &mut self.stroke {
            let last = *points.last().expect("strokes start with a point");
            if (last[0] - point[0]).hypot(last[1] - point[1]) >= 1.0 {
                points.push(point);
                self.map.segment(last, point, self.radius, color);
                changed = true;
            }
        }
        if released {
            // A drag that drew the whole shape is a lasso, so letting go fills it. One that
            // adds to placed points only closes if it ends back at the start.
            if let Some((_, began)) = self.shape_drag.take()
                && self.shape.len() >= 3
                && (began || on_first)
            {
                return self.close_shape(project);
            }
            if let Some(points) = self.stroke.take() {
                let kind = if self.tool == Tool::Erase {
                    StrokeKind::Erase
                } else {
                    StrokeKind::Brush
                };
                let stroke = Stroke {
                    kind,
                    normal: self.normal,
                    radius: self.radius,
                    points,
                };
                self.commit(project, stroke)?;
                changed = true;
            }
        }
        Ok(changed)
    }

    /// The selected piece's outline, the shape being drawn, and the brush.
    pub fn overlay(&self, ui: &egui::Ui, project: &Project, view: &View, rect: Rect, hover: Option<Pos2>) {
        let painting = [project.width as f32, project.height as f32];
        let placed = Placement::of(ui, view, rect, painting);
        let to_screen = |p: &[f32; 2]| placed.to_screen(*p);
        let painter = ui.painter_at(rect);
        let outline: Vec<Pos2> = project.cuts[self.piece].points.iter().map(to_screen).collect();
        painter.add(Shape::closed_line(
            outline.clone(),
            Line::new(3.0, Color32::WHITE),
        ));
        painter.add(Shape::closed_line(outline, Line::new(1.25, design::INK)));
        if self.tool == Tool::Shape {
            let dots: Vec<Pos2> = self.shape.iter().map(to_screen).collect();
            let mut points = dots.clone();
            if let Some(p) = hover.filter(|_| !dots.is_empty() && self.shape_drag.is_none()) {
                points.push(p);
            }
            painter.add(Shape::line(points.clone(), Line::new(3.0, Color32::WHITE)));
            painter.add(Shape::line(points, Line::new(1.25, design::INK)));
            for (i, dot) in dots.iter().enumerate() {
                let radius = if i == 0 { 5.0 } else { 3.5 };
                painter.circle(*dot, radius, Color32::WHITE, Line::new(1.5, design::INK));
            }
            if !dots.is_empty() {
                let hint = "Click the first point or press Return to fill. Esc to cancel.";
                design::toast(ui.ctx(), hint);
            }
            if let Some(i) = self.active {
                let strokes = &project.cuts[self.piece].strokes;
                let outline: Vec<Pos2> = strokes[i].points.iter().map(to_screen).collect();
                painter.add(Shape::closed_line(
                    outline.clone(),
                    Line::new(3.5, Color32::WHITE),
                ));
                painter.add(Shape::closed_line(outline.clone(), Line::new(1.5, design::INK)));
                for dot in &outline {
                    painter.circle(*dot, 3.5, Color32::WHITE, Line::new(1.5, design::INK));
                }
                let hint = "Pick a direction for this shape, then click to start the next one.";
                design::toast(ui.ctx(), hint);
            }
        } else if let Some(p) = hover {
            painter.circle_stroke(
                p,
                self.radius * placed.scale() + 0.75,
                Line::new(2.5, Color32::WHITE),
            );
            painter.circle_stroke(p, self.radius * placed.scale(), Line::new(1.0, design::INK));
        }
    }
}
