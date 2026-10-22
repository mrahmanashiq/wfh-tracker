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

fn app_icon() -> Result<Icon, PlatformError> {
    Icon::from_rgba(crate::icon_art::app_icon_rgba(32), 32, 32).map_err(tray_error)
}

fn tray_error(e: impl std::fmt::Display) -> PlatformError {
    PlatformError::Tray(e.to_string())
}
