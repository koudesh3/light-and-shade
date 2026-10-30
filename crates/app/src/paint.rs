//! Painting normals onto a piece, in the light room, under the real light.

use crate::ball::Sphere;
use crate::{canvas, design};
use anyhow::Result;
use eframe::egui::{self, Color32, Key, Pos2, Rect, Shape, Stroke as Line, vec2};
use las_core::{NormalMap, Project, Stroke, StrokeKind, View, encode_normal};

const HISTORY: usize = 100;
const FACING_YOU: [f32; 3] = [0.0, 0.0, 1.0];

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
    /// Points of the stroke under way, in source pixels.
    stroke: Option<Vec<[f32; 2]>>,
    sphere: Sphere,
    /// Earlier stroke lists, and the piece each belongs to.
    history: Vec<(usize, Vec<Stroke>)>,
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
            sphere: Sphere::default(),
            history: Vec::new(),
        }
    }

    pub fn map(&self) -> &NormalMap {
        &self.map
    }

    pub fn select(&mut self, project: &Project, piece: usize) {
        self.piece = piece;
        self.map = NormalMap::from_strokes(&project.layers[piece], &project.cuts[piece].strokes);
        self.stroke = None;
    }

    fn remember(&mut self, project: &Project) {
        self.history
            .push((self.piece, project.cuts[self.piece].strokes.clone()));
        if self.history.len() > HISTORY {
            self.history.remove(0);
        }
    }

    fn commit(&mut self, project: &mut Project, stroke: Stroke) -> Result<()> {
        self.remember(project);
        self.map.apply(&stroke);
        project.cuts[self.piece].strokes.push(stroke);
        project.store_normals(self.piece, &self.map)
    }

    fn undo(&mut self, project: &mut Project) -> Result<bool> {
        let Some((piece, strokes)) = self.history.pop() else {
            return Ok(false);
        };
        project.cuts[piece].strokes = strokes;
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
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), design::ROW), egui::Sense::hover());
        const SHADES: [Option<usize>; 3] = [None, Some(32), Some(16)];
        let selected = SHADES.iter().position(|s| *s == self.shades).unwrap_or(0);
        if let Some(i) = design::tabs(ui, rect.center(), &["All", "32", "16"], selected) {
            self.shades = SHADES[i];
        }
        design::gap(ui);
        design::heading(ui, "Tool");
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), design::ROW), egui::Sense::hover());
        const TOOLS: [Tool; 3] = [Tool::Brush, Tool::Shape, Tool::Erase];
        let selected = TOOLS.iter().position(|t| *t == self.tool).unwrap_or(0);
        if let Some(i) = design::tabs(ui, rect.center(), &["Brush", "Shape", "Erase"], selected) {
            self.tool = TOOLS[i];
        }
        ui.add_space(4.0);
        design::whole_slider(ui, "Size", &mut self.radius, 6.0..=300.0);
        ui.horizontal(|ui| {
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
            if !project.cuts[self.piece].strokes.is_empty() && design::button(ui, "Clear").clicked() {
                self.clear(project)?;
                changed = true;
            }
            anyhow::Ok(())
        })
        .inner?;
        Ok(changed)
    }

    pub fn keys(&mut self, ctx: &egui::Context, project: &mut Project) -> Result<bool> {
        if ctx.egui_wants_keyboard_input() {
            return Ok(false);
        }
        let undo = ctx.input(|i| {
            for (key, tool) in [
                (Key::B, Tool::Brush),
                (Key::L, Tool::Shape),
                (Key::E, Tool::Erase),
            ] {
                if i.key_pressed(key) {
                    self.tool = tool;
                }
            }
            if i.key_pressed(Key::OpenBracket) {
                self.radius = (self.radius - 10.0).max(6.0);
            }
            if i.key_pressed(Key::CloseBracket) {
                self.radius = (self.radius + 10.0).min(300.0);
            }
            i.modifiers.command && i.key_pressed(Key::Z)
        });
        if undo { self.undo(project) } else { Ok(false) }
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
        let (scale, origin) = canvas::placement(ui, view, rect, painting);
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
        let v = at - rect.min;
        let point = [(v.x - origin[0]) / scale, (v.y - origin[1]) / scale];
        let color = encode_normal(if self.tool == Tool::Erase {
            FACING_YOU
        } else {
            self.normal
        });
        let painting_line = self.tool != Tool::Shape;
        let mut changed = false;

        if pressed && inside {
            if alt {
                if let Some(normal) = self.map.sample(point) {
                    self.normal = normal;
                }
                return Ok(false);
            }
            self.stroke = Some(vec![point]);
            if painting_line {
                self.map.segment(point, point, self.radius, color);
                changed = true;
            }
        } else if down && let Some(points) = &mut self.stroke {
            let last = *points.last().expect("strokes start with a point");
            if (last[0] - point[0]).hypot(last[1] - point[1]) >= 1.0 {
                points.push(point);
                if painting_line {
                    self.map.segment(last, point, self.radius, color);
                    changed = true;
                }
            }
        }
        if released && let Some(points) = self.stroke.take() {
            let kind = match self.tool {
                Tool::Brush => StrokeKind::Brush,
                Tool::Erase => StrokeKind::Erase,
                Tool::Shape => StrokeKind::Shape,
            };
            if kind != StrokeKind::Shape || points.len() >= 3 {
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
        let (scale, origin) = canvas::placement(ui, view, rect, painting);
        let to_screen = |p: &[f32; 2]| rect.min + vec2(origin[0] + p[0] * scale, origin[1] + p[1] * scale);
        let painter = ui.painter_at(rect);
        let outline: Vec<Pos2> = project.cuts[self.piece].points.iter().map(to_screen).collect();
        painter.add(Shape::closed_line(
            outline.clone(),
            Line::new(3.0, Color32::WHITE),
        ));
        painter.add(Shape::closed_line(outline, Line::new(1.25, design::INK)));
        if self.tool == Tool::Shape {
            if let Some(points) = &self.stroke {
                let points: Vec<Pos2> = points.iter().map(to_screen).collect();
                painter.add(Shape::line(points.clone(), Line::new(3.0, Color32::WHITE)));
                painter.add(Shape::line(points, Line::new(1.25, design::INK)));
            }
        } else if let Some(p) = hover {
            painter.circle_stroke(p, self.radius * scale + 0.75, Line::new(2.5, Color32::WHITE));
            painter.circle_stroke(p, self.radius * scale, Line::new(1.0, design::INK));
        }
    }
}
