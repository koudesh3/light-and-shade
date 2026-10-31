//! The home page: a card for each painting, and a way to start a new one.

use crate::design;
use crate::studio::Nav;
use eframe::egui::{self, ColorImage, Pos2, TextureHandle, TextureOptions, Vec2, vec2};
use las_core::{Project, THUMBNAIL};
use std::path::Path;

const CARD: Vec2 = vec2(236.0, 178.0);

struct Card {
    project: Project,
    thumbnail: Option<TextureHandle>,
}

pub struct Home {
    cards: Vec<Card>,
    /// The card whose menu is open, and where the menu sits.
    menu: Option<(usize, Pos2)>,
    /// The card being renamed, and the name so far.
    renaming: Option<(usize, String)>,
    /// The name needs selecting for a few frames, until its text field exists.
    selecting_name: u8,
}

fn thumbnail(ctx: &egui::Context, project: &Project) -> Option<TextureHandle> {
    let path = project.dir.join(THUMBNAIL);
    if !path.is_file() {
        project.write_thumbnail().ok()?;
    }
    let image = image::open(path).ok()?.into_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let pixels = ColorImage::from_rgba_unmultiplied(size, image.as_raw());
    let name = project.dir.display().to_string();
    Some(ctx.load_texture(name, pixels, TextureOptions::LINEAR))
}

impl Home {
    pub fn new(ctx: &egui::Context, root: &Path) -> Self {
        let mut projects: Vec<Project> = std::fs::read_dir(root)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| Project::load(entry.path()).ok())
            .collect();
        projects.sort_by_key(|p| p.name.to_lowercase());
        let cards = projects
            .into_iter()
            .map(|project| Card {
                thumbnail: thumbnail(ctx, &project),
                project,
            })
            .collect();
        Self {
            cards,
            menu: None,
            renaming: None,
            selecting_name: 0,
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<Nav> {
        let mut nav = None;
        let mut opened = false;
        let mut titles = Vec::with_capacity(self.cards.len());
        let margin = egui::Margin {
            left: 64,
            right: 64,
            top: 56,
            bottom: 24,
        };
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(design::WHITE).inner_margin(margin))
            .show(ui, |ui| {
                design::title(ui, "Paintings");
                ui.add_space(20.0);
                egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(22.0, 26.0);
                        if design::new_card(ui, CARD, "New painting", "Drop an image anywhere").clicked() {
                            nav = pick_image().map(Nav::New);
                        }
                        for (i, card) in self.cards.iter().enumerate() {
                            let image = card.thumbnail.as_ref().map(|t| (t.id(), t.size_vec2()));
                            let menu_open = self.menu.is_some_and(|(open, _)| open == i);
                            let name = &card.project.name;
                            let response = design::card(ui, CARD, image, name, menu_open);
                            titles.push(response.title);
                            if let Some(at) = response.menu {
                                self.menu = if menu_open { None } else { Some((i, at)) };
                                opened = true;
                            } else if response.open.clicked() {
                                nav = Some(open(&card.project));
                            }
                        }
                    });
                });
            });
        if let Some((i, at)) = self.menu {
            match design::menu(ui.ctx(), at, &["Rename", "Move to Trash"]) {
                design::MenuChoice::Chosen(0) => {
                    self.renaming = Some((i, self.cards[i].project.name.clone()));
                    self.selecting_name = 2;
                    self.menu = None;
                }
                design::MenuChoice::Chosen(_) => {
                    nav = Some(Nav::Trash(self.cards[i].project.dir.clone()));
                    self.menu = None;
                }
                design::MenuChoice::Dismissed if !opened => self.menu = None,
                _ => {}
            }
        }
        if let Some((i, name)) = &mut self.renaming {
            let opening = self.selecting_name > 0;
            self.selecting_name = self.selecting_name.saturating_sub(1);
            match design::name_field(ui.ctx(), titles[*i], name, opening) {
                design::NameChoice::Save => {
                    let project = self.cards[*i].project.clone();
                    if *name != project.name {
                        nav = Some(Nav::Rename(project, name.clone()));
                    }
                    self.renaming = None;
                }
                design::NameChoice::Cancel => self.renaming = None,
                design::NameChoice::Waiting => {}
            }
        }
        nav
    }
}

/// Finished paintings open in the light room; ones with no pieces yet go to cutting.
fn open(project: &Project) -> Nav {
    if project.layers.is_empty() {
        Nav::Cut(project.clone())
    } else {
        Nav::Light(project.clone())
    }
}

pub fn pick_image() -> Option<std::path::PathBuf> {
    rfd::FileDialog::new()
        .set_title("Choose a painting")
        .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
        .pick_file()
}
