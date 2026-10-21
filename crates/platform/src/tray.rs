use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::PlatformError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayAction {
    Open,
    Quit,
}

/// The notification-area icon. Dropping it removes the icon.
pub struct Tray {
    _icon: TrayIcon,
}

impl Tray {
    /// Creates the icon. `on_action` runs on the OS event thread, so it must
    /// only forward the action (e.g. into the UI) and return quickly.
    ///
    /// On Windows this must be called on the thread that runs the window's
    /// event loop.
    pub fn create(
        on_action: impl Fn(TrayAction) + Send + Sync + 'static,
    ) -> Result<Self, PlatformError> {
        let open = MenuItem::new("Open WFH Tracker", true, None);
        let quit = MenuItem::new("Quit", true, None);
        let menu = Menu::new();
        menu.append_items(&[&open, &PredefinedMenuItem::separator(), &quit])
            .map_err(tray_error)?;

        let icon = TrayIconBuilder::new()
            .with_tooltip("WFH Tracker")
            .with_icon(app_icon()?)
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .build()
            .map_err(tray_error)?;

        let on_action = std::sync::Arc::new(on_action);
        let (open_id, quit_id) = (open.id().clone(), quit.id().clone());
        let menu_action = on_action.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if event.id == open_id {
                menu_action(TrayAction::Open);
            } else if event.id == quit_id {
                menu_action(TrayAction::Quit);
            }
        }));
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                on_action(TrayAction::Open);
            }
        }));

        Ok(Self { _icon: icon })
    }
}

/// A 32×32 blue disc with a white clock hand, drawn in code so the app ships no image files.
pub fn app_icon_rgba() -> (Vec<u8>, u32) {
    const SIZE: u32 = 32;
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    let c = (SIZE as f32 - 1.0) / 2.0;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let (dx, dy) = (x as f32 - c, y as f32 - c);
            let r = (dx * dx + dy * dy).sqrt();
            let hand = (dx.abs() < 1.5 && dy < 0.5 && dy > -c * 0.7)
                || (dy.abs() < 1.5 && dx > -0.5 && dx < c * 0.5);
            let px = match () {
                _ if r > c + 0.5 => [0, 0, 0, 0],
                _ if hand => [255, 255, 255, 255],
                _ => [0x3b, 0x82, 0xf6, 255],
            };
            rgba.extend_from_slice(&px);
        }
    }
    (rgba, SIZE)
}

fn app_icon() -> Result<Icon, PlatformError> {
    let (rgba, size) = app_icon_rgba();
    Icon::from_rgba(rgba, size, size).map_err(tray_error)
}

fn tray_error(e: impl std::fmt::Display) -> PlatformError {
    PlatformError::Tray(e.to_string())
}
