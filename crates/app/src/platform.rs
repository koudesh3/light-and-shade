//! The only OS-specific code in the app. Other platforms get the same commands
//! from the window's own controls, so this is optional polish.

/// Something the OS menu bar asked for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Command {
    New,
    Fit,
    Lit,
    Normals,
    Original,
    Orbit,
}

#[cfg(target_os = "macos")]
pub use mac::Menu;

#[cfg(not(target_os = "macos"))]
pub struct Menu;

#[cfg(not(target_os = "macos"))]
impl Menu {
    pub fn install() -> Self {
        Self
    }

    pub fn poll(&self) -> Option<Command> {
        None
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use super::Command;
    use muda::{Menu as Bar, MenuEvent, MenuItem, PredefinedMenuItem, Submenu, accelerator::Accelerator};

    pub struct Menu {
        _bar: Bar,
        items: Vec<(MenuItem, Command)>,
    }

    impl Menu {
        pub fn install() -> Self {
            let mut items = Vec::new();
            let mut item = |label: &str, key: Option<&str>, command| {
                let accelerator = key.map(|k| k.parse::<Accelerator>().expect("valid accelerator"));
                let item = MenuItem::new(label, true, accelerator);
                items.push((item.clone(), command));
                item
            };
            let new = item("New Painting…", Some("Cmd+N"), Command::New);
            let lit = item("Lit", Some("Cmd+1"), Command::Lit);
            let normals = item("Normals", Some("Cmd+2"), Command::Normals);
            let original = item("Original", Some("Cmd+3"), Command::Original);
            let fit = item("Fit to Window", Some("Cmd+0"), Command::Fit);
            let orbit = item("Orbit the Light", None, Command::Orbit);

            let separator = PredefinedMenuItem::separator;
            let app = Submenu::with_items(
                "Light and Shade",
                true,
                &[
                    &PredefinedMenuItem::about(None, None),
                    &separator(),
                    &PredefinedMenuItem::hide(None),
                    &PredefinedMenuItem::hide_others(None),
                    &separator(),
                    &PredefinedMenuItem::quit(None),
                ],
            );
            let file = Submenu::with_items("File", true, &[&new]);
            let view = Submenu::with_items(
                "View",
                true,
                &[&lit, &normals, &original, &separator(), &fit, &orbit],
            );
            let window = Submenu::with_items(
                "Window",
                true,
                &[
                    &PredefinedMenuItem::minimize(None),
                    &PredefinedMenuItem::fullscreen(None),
                ],
            );
            let bar = Bar::new();
            for menu in [app, file, view, window] {
                bar.append(&menu.expect("building a menu"))
                    .expect("adding a menu");
            }
            bar.init_for_nsapp();
            Self { _bar: bar, items }
        }

        pub fn poll(&self) -> Option<Command> {
            let event = MenuEvent::receiver().try_recv().ok()?;
            self.items
                .iter()
                .find(|(item, _)| *item.id() == event.id)
                .map(|(_, c)| *c)
        }
    }
}
