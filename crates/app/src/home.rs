//! The home page: a card for each painting, and a way to start a new one.

use crate::design;
use crate::studio::Nav;
use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions, Vec2, vec2};
use las_core::{Project, THUMBNAIL};
use std::path::Path;

const CARD: Vec2 = vec2(236.0, 178.0);

struct Card {
    project: Project,
    thumbnail: Option<TextureHandle>,
}

pub struct Home {
    cards: Vec<Card>,
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
        Self { cards }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<Nav> {
        let mut nav = None;
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
                        for card in &self.cards {
                            let image = card.thumbnail.as_ref().map(|t| (t.id(), t.size_vec2()));
                            let response =
                                design::card(ui, CARD, image, &card.project.name, &note(&card.project));
                            if response.clicked() {
                                nav = Some(open(&card.project));
                            }
                        }
                    });
                });
            });
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

fn note(project: &Project) -> String {
    match project.layers.len() {
        0 => "Not cut yet".into(),
        1 => "1 layer".into(),
        n => format!("{n} layers"),
    }
}

pub fn pick_image() -> Option<std::path::PathBuf> {
    rfd::FileDialog::new()
        .set_title("Choose a painting")
        .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
        .pick_file()
}
