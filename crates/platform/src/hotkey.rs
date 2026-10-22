use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

use crate::PlatformError;

/// The global Meeting hotkey, Ctrl+Alt+M. Dropping it unregisters the key.
pub struct MeetingHotkey {
    _manager: GlobalHotKeyManager,
}

impl MeetingHotkey {
    /// Registers the hotkey. `on_press` runs on the OS event thread, so it
    /// should only forward the press. On Windows, call this on the thread
    /// that runs the window's event loop.
    pub fn register(on_press: impl Fn() + Send + Sync + 'static) -> Result<Self, PlatformError> {
        let manager = GlobalHotKeyManager::new().map_err(hotkey_error)?;
        let hotkey = HotKey::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyM);
        manager.register(hotkey).map_err(hotkey_error)?;
        let id = hotkey.id();
        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            if event.id == id && event.state == HotKeyState::Pressed {
                on_press();
            }
        }));
        Ok(Self { _manager: manager })
    }
}

fn hotkey_error(e: impl std::fmt::Display) -> PlatformError {
    PlatformError::Hotkey(e.to_string())
}
