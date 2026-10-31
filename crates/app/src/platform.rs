//! The only OS-specific code in the app. Other platforms get the same commands
//! from the window's own controls, so this is optional polish.

/// Something the OS menu bar asked for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Command {
    New,
    Back,
    Fit,
    Lit,
    Normals,
    Original,
    Orbit,
}

/// Move a file or folder to the system Trash, where it can be recovered.
pub fn move_to_trash(path: &std::path::Path) -> Result<(), trash::Error> {
    #[cfg(target_os = "macos")]
    {
        use trash::macos::{DeleteMethod, TrashContextExtMacos};
        // Asking Finder instead would prompt for Automation permission.
        let mut context = trash::TrashContext::default();
        context.set_delete_method(DeleteMethod::NsFileManager);
        context.delete(path)
    }
    #[cfg(not(target_os = "macos"))]
    {
        trash::delete(path)
    }
}

#[cfg(target_os = "macos")]
pub use mac::{Menu, place_window_buttons};

/// Other platforms draw their window controls in their own title bars.
#[cfg(not(target_os = "macos"))]
pub fn place_window_buttons(_window: &eframe::Frame, _left: f64, _centre: f64) {}

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
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    use objc2_foundation::{NSPoint, NSRect};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    /// Set in a window's style mask while it's full screen.
    const FULL_SCREEN: usize = 1 << 14;

    /// Put the window buttons `left` points in and `centre` points down. macOS lays them out
    /// again on resize, so this runs every frame. Their spacing stays the system's.
    pub fn place_window_buttons(window: &eframe::Frame, left: f64, centre: f64) {
        let Ok(handle) = window.window_handle() else {
            return;
        };
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return;
        };
        let view = handle.ns_view.as_ptr().cast::<AnyObject>();
        // SAFETY: the view is alive while the frame is, and these are long-standing AppKit
        // messages on objects that AppKit owns. Each result is checked before it's used.
        unsafe {
            let window: *mut AnyObject = msg_send![view, window];
            if window.is_null() {
                return;
            }
            let style: usize = msg_send![window, styleMask];
            if style & FULL_SCREEN != 0 {
                return;
            }
            let button = |kind: usize| -> *mut AnyObject { msg_send![window, standardWindowButton: kind] };
            let buttons = [button(0), button(1), button(2)];
            if buttons.iter().any(|b| b.is_null()) {
                return;
            }
            let close: NSRect = msg_send![buttons[0], frame];
            let minimize: NSRect = msg_send![buttons[1], frame];
            let spacing = minimize.origin.x - close.origin.x;
            // The bar holding the buttons clips them, so it has to grow to reach them.
            let bar: *mut AnyObject = msg_send![buttons[0], superview];
            let container: *mut AnyObject = msg_send![bar, superview];
            if bar.is_null() || container.is_null() {
                return;
            }
            let outer: NSRect = msg_send![window, frame];
            let height = centre * 2.0;
            let mut area: NSRect = msg_send![container, frame];
            area.size.height = height;
            area.origin.y = outer.size.height - height;
            let _: () = msg_send![container, setFrame: area];
            let y = height / 2.0 - close.size.height / 2.0;
            for (i, button) in buttons.into_iter().enumerate() {
                let at = NSPoint::new(left + i as f64 * spacing, y);
                let _: () = msg_send![button, setFrameOrigin: at];
            }
        }
    }

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
            let back = item("Back to Paintings", Some("Cmd+["), Command::Back);
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
            let file = Submenu::with_items("File", true, &[&new, &back]);
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
