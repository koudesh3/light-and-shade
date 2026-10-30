//! The Light and Shade studio.

mod ball;
mod design;
mod library;
mod platform;
mod studio;

use eframe::egui;
use studio::{Scripted, Studio};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_title("Light and Shade")
            .with_inner_size([1360.0, 880.0])
            .with_min_inner_size([900.0, 560.0])
            .with_fullsize_content_view(true)
            .with_titlebar_shown(false)
            .with_title_shown(false),
        ..Default::default()
    };
    eframe::run_native(
        "Light and Shade",
        options,
        Box::new(|cc| {
            design::apply(&cc.egui_ctx);
            Ok(Box::new(Studio::new(cc, Scripted::from_env())?))
        }),
    )
}
