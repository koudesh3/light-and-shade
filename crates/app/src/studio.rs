//! The window: which screen is showing, how to get between them, and the slow work
//! that happens off the main thread.

use crate::cut::CutBoard;
use crate::home::{Home, pick_image};
use crate::light::LightRoom;
use crate::platform::{Command, Menu, place_window_buttons};
use crate::{design, library};
use anyhow::{Context, Result, anyhow};
use eframe::egui::{self, Event, Pos2};
use eframe::egui_wgpu::RenderState;
use las_core::{PACKAGE_EXTENSION, Project};
use las_render::Mode;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

/// Frames a scripted run waits before sending input.
const SETTLE_FRAMES: u32 = 10;

const APP_NAME: &str = "Light and Shade";

/// Where a screen wants to go next.
pub enum Nav {
    Home,
    Cut(Project),
    Light(Project),
    New(PathBuf),
    Bake(Project),
    /// Rebuild the layers, then go home.
    BakeHome(Project),
    /// Move a painting's folder to the Trash.
    Trash(PathBuf),
    Rename(Project, String),
    Export(Project, PathBuf),
}

enum Screen {
    Home(Home),
    Cut(Box<CutBoard>),
    Light(Box<LightRoom>),
}

#[derive(Clone, Copy)]
enum Then {
    Home,
    Cut,
    Light,
}

/// Slow work under way, and the screen to show when it's done.
struct Job {
    result: Receiver<Result<Project>>,
    then: Then,
    message: &'static str,
}

/// Settings and input for a scripted run, for capturing the window without OS permissions.
pub struct Scripted {
    shot: Option<PathBuf>,
    pub mode: Option<Mode>,
    pub zoom: Option<f32>,
    pub light_x: Option<f32>,
    open: Option<String>,
    drop: Option<PathBuf>,
    /// One batch of events per frame.
    input: VecDeque<Vec<Event>>,
}

fn points(text: &str) -> Vec<Pos2> {
    text.split(';')
        .filter_map(|pair| {
            let (x, y) = pair.split_once(',')?;
            Some(Pos2::new(x.trim().parse().ok()?, y.trim().parse().ok()?))
        })
        .collect()
}

impl Scripted {
    pub fn from_env() -> Self {
        let var = |name| std::env::var(name).ok();
        let button = |pos, pressed| Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        let mut input = VecDeque::new();
        // LAS_CLICKS: `x,y;x,y` clicks in window points. LAS_DRAG: press, move through, release.
        for p in points(&var("LAS_CLICKS").unwrap_or_default()) {
            input.extend([
                vec![Event::PointerMoved(p)],
                vec![button(p, true)],
                vec![button(p, false)],
                vec![],
            ]);
        }
        if let Some(lines) = var("LAS_SCROLL").and_then(|s| s.parse().ok()) {
            input.push_back(vec![
                Event::PointerMoved(Pos2::new(680.0, 440.0)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line,
                    delta: egui::vec2(0.0, lines),
                    phase: egui::TouchPhase::Move,
                    source: egui::MouseWheelSource::Wheel,
                    modifiers: Default::default(),
                },
            ]);
        }
        // LAS_PAN: `x,y` two-finger scroll in points. LAS_PINCH: a pinch zoom factor.
        let middle = Pos2::new(680.0, 440.0);
        if let Some(delta) = points(&var("LAS_PAN").unwrap_or_default()).first() {
            input.push_back(vec![
                Event::PointerMoved(middle),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: delta.to_vec2(),
                    phase: egui::TouchPhase::Move,
                    source: egui::MouseWheelSource::Trackpad,
                    modifiers: Default::default(),
                },
            ]);
        }
        if let Some(factor) = var("LAS_PINCH").and_then(|f| f.parse().ok()) {
            input.push_back(vec![Event::PointerMoved(middle), Event::Zoom(factor)]);
        }
        // LAS_TEXT: typing, after the input above.
        if let Some(text) = var("LAS_TEXT") {
            input.push_back(vec![Event::Text(text)]);
        }
        let drag = points(&var("LAS_DRAG").unwrap_or_default());
        if let (Some(first), Some(last)) = (drag.first(), drag.last()) {
            input.push_back(vec![Event::PointerMoved(*first)]);
            input.push_back(vec![button(*first, true)]);
            input.extend(drag.iter().map(|p| vec![Event::PointerMoved(*p)]));
            input.push_back(vec![button(*last, false)]);
        }
        // LAS_KEYS: shortcuts after everything else, such as `cmd+z;ctrl+shift+z;enter`.
        for shortcut in var("LAS_KEYS")
            .unwrap_or_default()
            .split(';')
            .filter(|s| !s.is_empty())
        {
            let has = |name| shortcut.split('+').any(|part| part == name);
            let modifiers = egui::Modifiers {
                ctrl: has("ctrl"),
                shift: has("shift"),
                mac_cmd: has("cmd"),
                command: has("cmd"),
                ..Default::default()
            };
            let name = match () {
                _ if has("enter") => egui::Key::Enter,
                _ if has("esc") => egui::Key::Escape,
                _ => egui::Key::Z,
            };
            let key = |pressed| Event::Key {
                key: name,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers,
            };
            input.extend([
                vec![Event::ModifiersChanged(modifiers), key(true)],
                vec![key(false), Event::ModifiersChanged(Default::default())],
                vec![],
            ]);
        }
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
            open: var("LAS_OPEN"),
            drop: var("LAS_DROP").map(PathBuf::from),
            input,
        }
    }
}

#[derive(Debug)]
struct ScriptedFile(PathBuf);

impl egui::DroppedFile for ScriptedFile {
    fn path(&self) -> &Path {
        &self.0
    }

    fn bytes(&self) -> Result<Vec<u8>, String> {
        std::fs::read(&self.0).map_err(|e| e.to_string())
    }
}

pub struct Studio {
    gpu: RenderState,
    root: PathBuf,
    screen: Screen,
    job: Option<Job>,
    menu: Menu,
    error: Option<String>,
    scripted: Scripted,
    frames: u32,
    settled: u32,
    title: String,
}

impl Studio {
    pub fn new(cc: &eframe::CreationContext, scripted: Scripted) -> Result<Self> {
        let gpu = cc
            .wgpu_render_state
            .clone()
            .ok_or_else(|| anyhow!("this window has no wgpu renderer"))?;
        let root = library::root()?;
        std::fs::create_dir_all(&root).with_context(|| format!("creating {}", root.display()))?;
        let screen = Screen::Home(Home::new(&cc.egui_ctx, &root));
        let mut studio = Self {
            gpu,
            root,
            screen,
            job: None,
            menu: Menu::install(),
            error: None,
            scripted,
            frames: 0,
            settled: 0,
            title: APP_NAME.to_owned(),
        };
        if let Some(name) = studio.scripted.open.take() {
            let project = Project::load(studio.root.join(name))?;
            let nav = if project.layers.is_empty() {
                Nav::Cut(project)
            } else {
                Nav::Light(project)
            };
            studio.go(&cc.egui_ctx, nav);
        }
        Ok(studio)
    }

    fn go(&mut self, ctx: &egui::Context, nav: Nav) {
        self.error = None;
        let result = match nav {
            Nav::Home => {
                self.screen = Screen::Home(Home::new(ctx, &self.root));
                Ok(())
            }
            Nav::Cut(project) => {
                CutBoard::new(ctx, project).map(|board| self.screen = Screen::Cut(Box::new(board)))
            }
            Nav::Light(project) => LightRoom::new(self.gpu.clone(), project, &self.scripted)
                .map(|room| self.screen = Screen::Light(Box::new(room))),
            Nav::New(file) => {
                let root = self.root.clone();
                if file
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case(PACKAGE_EXTENSION))
                {
                    self.spawn(ctx, Then::Home, "Opening your painting…", move || {
                        Project::import(&root, &file)
                    });
                } else {
                    self.spawn(ctx, Then::Cut, "Preparing your painting…", move || {
                        Project::create(&root, &file)
                    });
                }
                Ok(())
            }
            Nav::Export(project, path) => {
                self.spawn(ctx, Then::Home, "Exporting your painting…", move || {
                    project.export(&path)?;
                    Ok(project)
                });
                Ok(())
            }
            Nav::Bake(project) => {
                self.bake(ctx, Then::Light, project);
                Ok(())
            }
            Nav::Trash(dir) => {
                let trashed = crate::platform::move_to_trash(&dir)
                    .with_context(|| format!("moving {} to the Trash", dir.display()));
                self.screen = Screen::Home(Home::new(ctx, &self.root));
                trashed
            }
            Nav::Rename(mut project, name) => {
                let renamed = project.rename(&name);
                self.screen = Screen::Home(Home::new(ctx, &self.root));
                renamed
            }
            Nav::BakeHome(project) => {
                self.bake(ctx, Then::Home, project);
                Ok(())
            }
        };
        self.error = result.err().map(|e| format!("{e:#}"));
    }

    fn bake(&mut self, ctx: &egui::Context, then: Then, mut project: Project) {
        self.spawn(ctx, then, "Cutting out your pieces…", move || {
            project.bake()?;
            Ok(project)
        });
    }

    fn spawn(
        &mut self,
        ctx: &egui::Context,
        then: Then,
        message: &'static str,
        work: impl FnOnce() -> Result<Project> + Send + 'static,
    ) {
        let (send, result) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = send.send(work());
            ctx.request_repaint();
        });
        self.job = Some(Job {
            result,
            then,
            message,
        });
    }

    fn finish_job(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.job else { return };
        let outcome = match job.result.try_recv() {
            Ok(outcome) => outcome,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err(anyhow!("the work stopped unexpectedly")),
        };
        let then = job.then;
        self.job = None;
        match (outcome, then) {
            (Ok(_), Then::Home) => self.go(ctx, Nav::Home),
            (Ok(project), Then::Cut) => self.go(ctx, Nav::Cut(project)),
            (Ok(project), Then::Light) => self.go(ctx, Nav::Light(project)),
            (Err(e), _) => self.error = Some(format!("{e:#}")),
        }
    }

    /// Keep the real window title current, for Mission Control and the Window menu.
    fn sync_title(&mut self, ctx: &egui::Context) {
        let title = match &self.screen {
            Screen::Home(_) => APP_NAME,
            Screen::Cut(board) => board.title(),
            Screen::Light(room) => room.title(),
        };
        if self.title != title {
            self.title = title.to_owned();
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(self.title.clone()));
        }
    }

    fn menu_commands(&mut self, ctx: &egui::Context) {
        while let Some(command) = self.menu.poll() {
            match (command, &mut self.screen) {
                (Command::New, _) => {
                    if let Some(image) = pick_image() {
                        self.go(ctx, Nav::New(image));
                    }
                }
                (Command::Back, Screen::Light(room)) => {
                    let nav = room.leave();
                    self.go(ctx, nav);
                }
                (Command::Back, Screen::Cut(board)) => {
                    let nav = board.leave();
                    self.go(ctx, nav);
                }
                (command, Screen::Light(room)) => room.command(command),
                _ => {}
            }
        }
    }

    /// An image dropped on the home page starts a painting.
    fn file_drop(&mut self, ctx: &egui::Context) -> Option<Nav> {
        if !matches!(self.screen, Screen::Home(_)) || self.job.is_some() {
            return None;
        }
        let (hovering, dropped) = ctx.input(|i| {
            let dropped = i.raw.dropped_files.first().map(|f| f.path().to_path_buf());
            (!i.raw.hovered_files.is_empty(), dropped)
        });
        if hovering {
            design::drop_veil(ctx, "Drop to add a painting");
        }
        dropped.map(Nav::New)
    }

    fn capture(&mut self, ctx: &egui::Context) {
        let Some(path) = self.scripted.shot.clone() else {
            return;
        };
        ctx.request_repaint();
        if self.job.is_some() || !self.scripted.input.is_empty() || self.scripted.drop.is_some() {
            self.settled = 0;
            return;
        }
        self.settled += 1;
        if self.settled == 20 {
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
        if self.scripted.shot.is_none() || self.settled > 0 && self.scripted.input.is_empty() {
            return;
        }
        if let Some(path) = self.scripted.drop.take() {
            input.dropped_files.push(Arc::new(ScriptedFile(path)));
        }
        // Widgets answer to the layout of the previous frame, so let the window settle first.
        self.frames += 1;
        if self.frames > SETTLE_FRAMES
            && self.job.is_none()
            && let Some(events) = self.scripted.input.pop_front()
        {
            input.events.extend(events);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        place_window_buttons(
            frame,
            design::WINDOW_BUTTONS_LEFT as f64,
            design::STRIP_CENTRE as f64,
        );
        self.finish_job(&ctx);
        self.menu_commands(&ctx);
        let nav = match &mut self.screen {
            Screen::Home(home) => home.ui(ui),
            Screen::Cut(board) => board.ui(ui),
            Screen::Light(room) => room.ui(ui),
        };
        let nav = nav.or_else(|| self.file_drop(&ctx));
        if let Some(job) = &self.job {
            design::toast(&ctx, job.message);
            ctx.request_repaint();
        } else if let Some(error) = &self.error {
            design::toast(&ctx, error);
        }
        if let Some(nav) = nav {
            self.go(&ctx, nav);
        }
        self.sync_title(&ctx);
        self.capture(&ctx);
    }
}
