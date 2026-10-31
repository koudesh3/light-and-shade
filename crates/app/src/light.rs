//! The light room: a painting's layers on the left, the lit painting in the middle,
//! the light on the right.

use crate::paint::Painter;
use crate::platform::Command;
use crate::studio::{Nav, Scripted};
use crate::{ball::Ball, design};
use anyhow::{Result, anyhow};
use eframe::egui::{self, Color32, Key, Pos2, Rect, Sense, TextureId};
use eframe::egui_wgpu::RenderState;
use las_core::{Light, Project, View};
use las_render::{Frame, Mode, Renderer, TARGET_FORMAT};

/// Room beside the right panel's cards for its scroll bar.
const GUTTER: f32 = 8.0;

struct Target {
    view: wgpu::TextureView,
    id: TextureId,
    size: [u32; 2],
}

pub struct LightRoom {
    gpu: RenderState,
    renderer: Renderer,
    target: Option<Target>,
    project: Project,
    visible: Vec<bool>,
    light: Light,
    ambient: f32,
    strength: f32,
    sheen: f32,
    mode: Mode,
    linear: bool,
    normal_opacity: f32,
    dark_paper: bool,
    orbit: bool,
    view: View,
    ball: Ball,
    /// Normals changed, so the home page cover needs redrawing.
    painted: bool,
    /// Dragging pans the painting, for when there's no keyboard to hold Space on.
    moving: bool,
    /// The layer being renamed.
    naming: Option<design::InlineName>,
    /// Present while painting normals.
    painter: Option<Painter>,
    error: Option<String>,
}

impl LightRoom {
    pub fn new(gpu: RenderState, project: Project, scripted: &Scripted) -> Result<Self> {
        if project.layers.is_empty() {
            return Err(anyhow!("this painting has no pieces yet"));
        }
        let mut renderer = Renderer::new(&gpu.device);
        renderer.load(&gpu.device, &gpu.queue, &project)?;
        // Fit the pieces rather than the photo they came from, so they fill the canvas.
        let view = View {
            zoom: scripted.zoom.unwrap_or(1.0),
            ..View::framing(project.layer_bounds())
        };
        Ok(Self {
            visible: vec![true; project.layers.len()],
            gpu,
            renderer,
            target: None,
            project,
            light: Light {
                x: scripted.light_x.unwrap_or(-0.55),
                y: 0.5,
                height: 0.55,
            },
            ambient: 0.34,
            strength: 0.95,
            sheen: 0.08,
            mode: scripted.mode.unwrap_or(Mode::Lit),
            linear: false,
            normal_opacity: 0.5,
            dark_paper: false,
            orbit: false,
            view,
            ball: Ball::default(),
            painted: false,
            moving: false,
            naming: None,
            painter: None,
            error: None,
        })
    }

    pub fn title(&self) -> &str {
        &self.project.name
    }

    pub fn command(&mut self, command: Command) {
        match command {
            Command::Fit => self.view = View::framing(self.project.layer_bounds()),
            Command::Lit => self.mode = Mode::Lit,
            Command::Normals => self.mode = Mode::Normals,
            Command::Original => self.mode = Mode::Original,
            Command::Orbit => self.orbit = !self.orbit,
            Command::New | Command::Back => {}
        }
    }

    fn painting_size(&self) -> [f32; 2] {
        [self.project.width as f32, self.project.height as f32]
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) -> Option<Nav> {
        const MODES: [Mode; 3] = [Mode::Lit, Mode::Normals, Mode::Original];
        let rect = ui.max_rect();
        let selected = MODES.iter().position(|m| *m == self.mode).unwrap_or(0);
        if let Some(i) = design::tabs(ui, rect.center(), &["Lit", "Normals", "Original"], selected) {
            self.mode = MODES[i];
        }
        let mut nav = None;
        ui.horizontal_centered(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let paintable = self.project.is_paintable();
                let cuttable = self.project.is_cuttable();
                let count = 1 + paintable as usize + cuttable as usize;
                design::card_tray(ui, design::INSPECTOR_CARD, count, |ui, cell| {
                    if cuttable && design::cell_button(ui, "Edit cuts", false, cell).clicked() {
                        nav = Some(Nav::Cut(self.project.clone()));
                    }
                    let painting = self.painter.is_some();
                    if paintable && design::cell_button(ui, "Paint", painting, cell).clicked() {
                        self.painter = (!painting).then(|| Painter::new(&self.project));
                    }
                    if design::cell_button(ui, "Move", self.moving, cell).clicked() {
                        self.moving = !self.moving;
                    }
                });
            });
        });
        nav
    }

    fn layers(&mut self, ui: &mut egui::Ui) {
        design::group(ui, |ui| {
            design::heading(ui, "Layers");
            ui.spacing_mut().item_spacing.y = 0.0;
            for i in (0..self.project.layers.len()).rev() {
                let name = &self.project.layers[i].name;
                let selected = self.painter.as_ref().is_some_and(|p| p.piece == i);
                let (row, eye) = design::layer_row(ui, name, self.visible[i], selected);
                if let Some(painter) = self.painter.as_mut().filter(|_| row.clicked()) {
                    painter.select(&self.project, i);
                }
                if row.double_clicked() {
                    self.naming = Some(design::InlineName::over_row(i, row.rect, name));
                }
                if eye {
                    self.visible[i] = !self.visible[i];
                }
            }
        });
    }

    /// Send the piece being painted to the GPU.
    fn upload_painted(&mut self) {
        if let Some(painter) = &self.painter {
            let map = &painter.map().image;
            let size = [map.width(), map.height()];
            self.renderer.set_normal(
                &self.gpu.device,
                &self.gpu.queue,
                painter.piece,
                size,
                map.as_raw(),
            );
        }
    }

    fn after_paint(&mut self, outcome: anyhow::Result<bool>) {
        match outcome {
            Ok(true) => {
                self.painted = true;
                self.upload_painted();
            }
            Ok(false) => {}
            Err(e) => self.error = Some(format!("{e:#}")),
        }
    }

    fn right_panel(&mut self, ui: &mut egui::Ui) {
        if let Some(painter) = &mut self.painter {
            let outcome = painter.panel(ui, &mut self.project);
            self.after_paint(outcome);
            design::gap(ui);
        }
        design::group(ui, |ui| {
            design::heading(ui, "Light");
            ui.add_space(4.0);
            ui.vertical_centered(|ui| {
                if self
                    .ball
                    .show(ui, &mut self.light, self.ambient, self.strength, self.sheen)
                {
                    self.orbit = false;
                }
            });
            ui.add_space(8.0);
            design::slider(ui, "Height", &mut self.light.height, 0.05..=1.0);
            design::slider(ui, "Strength", &mut self.strength, 0.0..=2.0);
            design::slider(ui, "Ambient", &mut self.ambient, 0.0..=1.0);
            design::slider(ui, "Sheen", &mut self.sheen, 0.0..=1.0);
            design::toggle(ui, "Orbit", &mut self.orbit);
        });
        design::gap(ui);
        design::group(ui, |ui| {
            design::heading(ui, "Display");
            design::toggle(ui, "Dark paper", &mut self.dark_paper);
            design::toggle(ui, "Linear light", &mut self.linear);
        });
        design::gap(ui);
        self.layers(ui);
    }

    fn viewport(&mut self, ui: &mut egui::Ui) {
        let rect = ui.max_rect();
        let response = ui.allocate_rect(rect, Sense::click_and_drag());
        let ppp = ui.pixels_per_point();
        let target = [
            (rect.width() * ppp).round().max(1.0),
            (rect.height() * ppp).round().max(1.0),
        ];
        let painting = self.painting_size();
        let panning = self.moving
            || ui.input(|i| i.key_down(Key::Space) || i.pointer.middle_down() || i.pointer.secondary_down());

        let badge = design::zoom_badge_rect(ui, rect, self.view.zoom);
        let over_badge = ui
            .input(|i| i.pointer.interact_pos())
            .is_some_and(|p| badge.contains(p));
        if over_badge || self.naming.is_some() {
            // The badge has its own click, so nothing here should paint or move the light.
        } else if panning {
            // Stay here for the whole press. A drag only registers once the pointer has
            // moved, and falling through before that would leave a dot of paint.
            if response.dragged() {
                let d = response.drag_delta() * ppp;
                self.view.pan[0] += d.x;
                self.view.pan[1] += d.y;
            }
        } else if let Some(painter) = &mut self.painter {
            let outcome = painter.pointer(
                ui,
                &mut self.project,
                &self.view,
                rect,
                response.contains_pointer(),
            );
            self.after_paint(outcome);
        } else if let Some(p) = response
            .interact_pointer_pos()
            .filter(|_| response.is_pointer_button_down_on())
        {
            // Drag across the painting to move the light, as in the browser study.
            let half = rect.width().min(rect.height()) / 2.0;
            self.light.x = (p.x - rect.center().x) / half * 1.4;
            self.light.y = -(p.y - rect.center().y) / half * 1.4;
            self.orbit = false;
        }
        if response.double_clicked() && self.painter.is_none() {
            self.view = View::framing(self.project.layer_bounds());
        }
        if let Some(p) = response.hover_pos() {
            crate::canvas::navigate(ui, &mut self.view, p, rect, painting);
        }

        let (id, size) = self.draw(target);
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        debug_assert_eq!(size, [target[0] as u32, target[1] as u32]);
        ui.painter().image(id, rect, uv, Color32::WHITE);
        if let Some(painter) = &self.painter {
            let brush = response.hover_pos().filter(|_| !panning && !over_badge);
            painter.overlay(ui, &self.project, &self.view, rect, brush);
        }
        design::canvas_corners(ui, rect);
        if design::zoom_badge(ui, badge, self.view.zoom).clicked() {
            self.view = View::framing(self.project.layer_bounds());
        }
        if response.hovered() {
            let icon = match (panning, self.painter.is_some()) {
                (true, _) if response.dragged() => egui::CursorIcon::Grabbing,
                (true, _) => egui::CursorIcon::Grab,
                (false, true) => egui::CursorIcon::Crosshair,
                (false, false) => egui::CursorIcon::Default,
            };
            ui.ctx().set_cursor_icon(icon);
        }
    }

    /// Draw the painting offscreen for egui to show as an image.
    fn draw(&mut self, target: [f32; 2]) -> (TextureId, [u32; 2]) {
        let size = [target[0] as u32, target[1] as u32];
        if self.target.as_ref().is_none_or(|t| t.size != size) {
            self.resize(size);
        }
        let t = self.target.as_ref().expect("resize made a target");
        let background = if self.dark_paper {
            [0.07, 0.086, 0.075]
        } else {
            [1.0, 1.0, 1.0]
        };
        let frame = Frame {
            transform: self.view.transform(target, self.painting_size()),
            light: self.light.direction(),
            ambient: self.ambient,
            strength: self.strength,
            sheen: self.sheen,
            mode: self.mode,
            linear: self.linear,
            normal_opacity: self.normal_opacity,
            background,
            visible: &self.visible,
        };
        let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
        self.renderer
            .render(&self.gpu.queue, &mut encoder, &t.view, &frame);
        self.gpu.queue.submit([encoder.finish()]);
        (t.id, t.size)
    }

    fn resize(&mut self, size: [u32; 2]) {
        let texture = self.gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("viewport"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TARGET_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let mut egui_renderer = self.gpu.renderer.write();
        let filter = wgpu::FilterMode::Nearest;
        let id = match self.target.take() {
            Some(old) => {
                egui_renderer.update_egui_texture_from_wgpu_texture(&self.gpu.device, &view, filter, old.id);
                old.id
            }
            None => egui_renderer.register_native_texture(&self.gpu.device, &view, filter),
        };
        self.target = Some(Target { view, id, size });
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        ctx.input(|i| {
            for (key, mode) in [
                (Key::Num1, Mode::Lit),
                (Key::Num2, Mode::Normals),
                (Key::Num3, Mode::Original),
            ] {
                if i.key_pressed(key) {
                    self.mode = mode;
                }
            }
            if i.key_pressed(Key::F) {
                self.view = View::framing(self.project.layer_bounds());
            }
            if i.key_pressed(Key::O) {
                self.orbit = !self.orbit;
            }
            if i.key_pressed(Key::M) {
                self.moving = !self.moving;
            }
        });
    }

    /// Go back to the home page, with a fresh cover if anything was painted.
    pub fn leave(&mut self) -> Nav {
        if self.painted
            && let Err(e) = self.project.write_thumbnail()
        {
            self.error = Some(format!("{e:#}"));
        }
        Nav::Home
    }

    fn finish_naming(&mut self, ctx: &egui::Context) {
        let Some(naming) = &mut self.naming else { return };
        match naming.show(ctx) {
            design::NameChoice::Waiting => {}
            design::NameChoice::Cancel => self.naming = None,
            design::NameChoice::Save => {
                let (index, text) = (naming.index, naming.text.clone());
                self.naming = None;
                if text != self.project.layers[index].name {
                    let renamed = self.project.rename_layer(index, &text);
                    self.error = renamed.err().map(|e| format!("{e:#}"));
                }
            }
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<Nav> {
        let ctx = ui.ctx().clone();
        self.shortcuts(&ctx);
        if let Some(painter) = &mut self.painter {
            let outcome = painter.keys(&ctx, &mut self.project);
            self.after_paint(outcome);
        }
        if let Some(error) = &self.error {
            design::toast(&ctx, error);
        }
        if self.orbit {
            let a = ctx.input(|i| i.time) as f32 * 0.6;
            self.light.x = a.cos() * 0.8;
            self.light.y = a.sin() * 0.8;
            ctx.request_repaint();
        }
        let title = self.project.name.clone();
        let (nav, back) = design::top_bar(ui, &title, |ui| self.toolbar(ui));
        let nav = if back { Some(self.leave()) } else { nav };
        design::side_panel_with_gutter(
            egui::Panel::right("inspector"),
            design::INSPECTOR,
            GUTTER as i8,
            ui,
            |ui| {
                // Pinned above the scrolling part, so it can't scroll out of reach while painting.
                if self.mode == Mode::Normals || self.painter.is_some() {
                    ui.scope(|ui| {
                        ui.set_max_width(ui.available_width() - GUTTER);
                        design::group(ui, |ui| {
                            let changed =
                                design::slider(ui, "Normal opacity", &mut self.normal_opacity, 0.0..=1.0);
                            if changed {
                                self.mode = Mode::Normals;
                            }
                        });
                    });
                    design::gap(ui);
                }
                egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                    // Keep a gutter for the scroll bar, so it never sits on the values.
                    ui.set_max_width(ui.available_width() - GUTTER);
                    self.right_panel(ui)
                });
            },
        );
        egui::CentralPanel::default()
            .frame(design::canvas_frame(false, true))
            .show(ui, |ui| self.viewport(ui));
        self.finish_naming(&ctx);
        nav
    }
}

impl Drop for LightRoom {
    fn drop(&mut self) {
        if let Some(target) = &self.target {
            self.gpu.renderer.write().free_texture(&target.id);
        }
    }
}
