//! The studio window: library and layers on the left, the lit painting in the middle,
//! the light on the right.

use crate::platform::{Command, Menu};
use crate::{ball::Ball, design, library};
use anyhow::{Context, Result, anyhow};
use eframe::egui::{self, Color32, Key, Pos2, Rect, Sense, TextureId};
use eframe::egui_wgpu::RenderState;
use las_core::{Light, Project, View};
use las_render::{Frame, Mode, Renderer, TARGET_FORMAT};
use std::path::PathBuf;

/// Settings a scripted run applies, for capturing the window without any OS permissions.
pub struct Scripted {
    shot: Option<PathBuf>,
    mode: Option<Mode>,
    zoom: Option<f32>,
    light_x: Option<f32>,
    /// Press at the first point, drag to the second, release; in window points.
    drag: Option<[f32; 4]>,
    /// Scroll over the middle of the window.
    scroll: Option<f32>,
}

impl Scripted {
    pub fn from_env() -> Self {
        let var = |name| std::env::var(name).ok();
        Self {
            shot: var("LAS_SHOT").map(PathBuf::from),
            mode: var("LAS_MODE").and_then(|m| match m.as_str() {
                "lit" => Some(Mode::Lit),
                "normals" => Some(Mode::Normals),
                "original" => Some(Mode::Original),
                _ => None,
            }),
            zoom: var("LAS_ZOOM").and_then(|z| z.parse().ok()),
            light_x: var("LAS_LIGHT_X").and_then(|x| x.parse().ok()),
            drag: var("LAS_DRAG").and_then(|d| {
                let v: Vec<f32> = d.split(',').filter_map(|n| n.parse().ok()).collect();
                v.try_into().ok()
            }),
            scroll: var("LAS_SCROLL").and_then(|s| s.parse().ok()),
        }
    }
}

struct Target {
    view: wgpu::TextureView,
    id: TextureId,
    size: [u32; 2],
}

pub struct Studio {
    gpu: RenderState,
    renderer: Renderer,
    target: Option<Target>,
    root: Option<PathBuf>,
    library: Vec<(PathBuf, String)>,
    project: Project,
    visible: Vec<bool>,
    light: Light,
    ambient: f32,
    strength: f32,
    sheen: f32,
    mode: Mode,
    linear: bool,
    dark_paper: bool,
    orbit: bool,
    view: View,
    ball: Ball,
    error: Option<String>,
    scripted: Scripted,
    frames: u32,
    menu: Menu,
}

impl Studio {
    pub fn new(cc: &eframe::CreationContext, scripted: Scripted) -> Result<Self> {
        let gpu = cc
            .wgpu_render_state
            .clone()
            .ok_or_else(|| anyhow!("this window has no wgpu renderer"))?;
        let root = library::root();
        let library = root.as_deref().map(library::scan).unwrap_or_default();
        let first = library
            .first()
            .context("no projects found: set LAS_PROJECTS to a folder of paintings")?;
        let project = Project::load(&first.0)?;
        let mut renderer = Renderer::new(&gpu.device);
        renderer.load(&gpu.device, &gpu.queue, &project)?;
        let mut studio = Self {
            visible: vec![true; project.layers.len()],
            gpu,
            renderer,
            target: None,
            root,
            library,
            project,
            light: Light {
                x: -0.55,
                y: 0.5,
                height: 0.55,
            },
            ambient: 0.34,
            strength: 0.95,
            sheen: 0.08,
            mode: Mode::Lit,
            linear: false,
            dark_paper: false,
            orbit: false,
            view: View::default(),
            ball: Ball::default(),
            error: None,
            scripted,
            frames: 0,
            menu: Menu::install(),
        };
        if let Some(mode) = studio.scripted.mode {
            studio.mode = mode;
        }
        if let Some(zoom) = studio.scripted.zoom {
            studio.view.zoom = zoom;
        }
        if let Some(x) = studio.scripted.light_x {
            studio.light.x = x;
        }
        Ok(studio)
    }

    fn open(&mut self, dir: PathBuf) {
        let opened = Project::load(&dir).and_then(|project| {
            self.renderer.load(&self.gpu.device, &self.gpu.queue, &project)?;
            Ok(project)
        });
        match opened {
            Ok(project) => {
                self.visible = vec![true; project.layers.len()];
                self.view = View::default();
                self.project = project;
                self.error = None;
            }
            Err(e) => self.error = Some(format!("{e:#}")),
        }
    }

    fn pick_folder(&self) -> Option<PathBuf> {
        let mut dialog = rfd::FileDialog::new().set_title("Open a painting folder");
        if let Some(root) = &self.root {
            dialog = dialog.set_directory(root);
        }
        dialog.pick_folder()
    }

    fn menu_commands(&mut self) {
        while let Some(command) = self.menu.poll() {
            match command {
                Command::Open => {
                    if let Some(dir) = self.pick_folder() {
                        self.open(dir);
                    }
                }
                Command::Fit => self.view = View::default(),
                Command::Lit => self.mode = Mode::Lit,
                Command::Normals => self.mode = Mode::Normals,
                Command::Original => self.mode = Mode::Original,
                Command::Orbit => self.orbit = !self.orbit,
            }
        }
    }

    fn painting_size(&self) -> [f32; 2] {
        [self.project.width as f32, self.project.height as f32]
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        const MODES: [Mode; 3] = [Mode::Lit, Mode::Normals, Mode::Original];
        let rect = ui.max_rect();
        let selected = MODES.iter().position(|m| *m == self.mode).unwrap_or(0);
        if let Some(i) = design::tabs(ui, rect.center(), &["Lit", "Normals", "Original"], selected) {
            self.mode = MODES[i];
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if design::button(ui, &format!("{:.0}%", self.view.zoom * 100.0)).clicked() {
                self.view = View::default();
            }
        });
    }

    fn left_panel(&mut self, ui: &mut egui::Ui) {
        design::heading(ui, "Paintings");
        let mut open = None;
        for (dir, name) in &self.library {
            let current = *dir == self.project.dir;
            if design::row(ui, name, current, design::INK).clicked() && !current {
                open = Some(dir.clone());
            }
        }
        if design::row(ui, "Open folder…", false, design::MUTED).clicked() {
            open = self.pick_folder();
        }
        if let Some(dir) = open {
            self.open(dir);
        }
        if let Some(e) = &self.error {
            ui.colored_label(Color32::from_rgb(190, 60, 40), e);
        }
        design::gap(ui);
        design::heading(ui, "Layers");
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for i in (0..self.project.layers.len()).rev() {
                let name = &self.project.layers[i].name;
                if design::layer_row(ui, name, self.visible[i]).clicked() {
                    self.visible[i] = !self.visible[i];
                }
            }
        });
    }

    fn right_panel(&mut self, ui: &mut egui::Ui) {
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
        design::gap(ui);
        design::heading(ui, "Display");
        design::toggle(ui, "Dark paper", &mut self.dark_paper);
        design::toggle(ui, "Linear light", &mut self.linear);
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
        let panning =
            ui.input(|i| i.key_down(Key::Space) || i.pointer.middle_down() || i.pointer.secondary_down());

        if response.dragged() && panning {
            let d = response.drag_delta() * ppp;
            self.view.pan[0] += d.x;
            self.view.pan[1] += d.y;
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
        if response.double_clicked() {
            self.view = View::default();
        }
        if let Some(p) = response.hover_pos() {
            let (scroll, pinch) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let factor = pinch * (scroll * 0.002).exp();
            if factor != 1.0 {
                let at = (p - rect.min) * ppp;
                self.view.zoom_at(factor, [at.x, at.y], target, painting);
            }
        }

        let (id, size) = self.draw(target);
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        debug_assert_eq!(size, [target[0] as u32, target[1] as u32]);
        ui.painter().image(id, rect, uv, Color32::WHITE);
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
                self.view = View::default();
            }
            if i.key_pressed(Key::O) {
                self.orbit = !self.orbit;
            }
        });
    }

    fn capture(&mut self, ctx: &egui::Context) {
        self.frames += 1;
        let Some(path) = self.scripted.shot.clone() else {
            return;
        };
        ctx.request_repaint();
        if self.frames == 30 {
            let callback = egui::ScreenshotCallback::new(move |image| {
                let [w, h] = image.size;
                let rgba: Vec<u8> = image
                    .pixels
                    .iter()
                    .flat_map(|c| c.to_srgba_unmultiplied())
                    .collect();
                match image::save_buffer(&path, &rgba, w as u32, h as u32, image::ColorType::Rgba8) {
                    Ok(()) => std::process::exit(0),
                    Err(e) => {
                        eprintln!("saving {}: {e}", path.display());
                        std::process::exit(1)
                    }
                }
            });
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(callback));
        }
    }
}

impl eframe::App for Studio {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, input: &mut egui::RawInput) {
        use egui::{Event, PointerButton};
        let button = |pos, pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        if let Some([x0, y0, x1, y1]) = self.scripted.drag {
            let (from, to) = (Pos2::new(x0, y0), Pos2::new(x1, y1));
            match self.frames {
                2 => input.events.push(Event::PointerMoved(from)),
                3 => input.events.push(button(from, true)),
                4 => input.events.push(Event::PointerMoved(to)),
                5 => input.events.push(button(to, false)),
                _ => {}
            }
        }
        if let (Some(lines), 3) = (self.scripted.scroll, self.frames) {
            let middle = input.screen_rect.map_or(Pos2::new(600.0, 400.0), |r| r.center());
            input.events.push(Event::PointerMoved(middle));
            input.events.push(Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: egui::vec2(0.0, lines),
                phase: egui::TouchPhase::Move,
                source: egui::MouseWheelSource::Wheel,
                modifiers: Default::default(),
            });
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.menu_commands();
        self.shortcuts(&ctx);
        if self.orbit {
            let a = ctx.input(|i| i.time) as f32 * 0.6;
            self.light.x = a.cos() * 0.8;
            self.light.y = a.sin() * 0.8;
            ctx.request_repaint();
        }
        let page = |x, y| {
            egui::Frame::new()
                .fill(design::WHITE)
                .inner_margin(egui::Margin::symmetric(x, y))
        };
        egui::Panel::top("toolbar")
            .exact_size(46.0)
            .show_separator_line(false)
            .frame(page(design::PAGE, 0))
            .show(ui, |ui| self.toolbar(ui));
        egui::Panel::left("sidebar")
            .exact_size(216.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(page(design::PAGE, 4))
            .show(ui, |ui| self.left_panel(ui));
        egui::Panel::right("inspector")
            .exact_size(244.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(page(design::PAGE, 4))
            .show(ui, |ui| self.right_panel(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(design::WHITE))
            .show(ui, |ui| self.viewport(ui));
        self.capture(&ctx);
    }
}
