use std::path::Path;

use wt_core::Notifier;

use crate::PlatformError;

/// The name Windows knows the app's notifications by (its AppUserModelID).
pub const APP_ID: &str = "com.mrahmanashiq.wfh-tracker";

/// Registers WFH Tracker as a notification sender for the current user, so
/// toasts show "WFH Tracker" with its icon (instead of "Windows PowerShell")
/// and can be managed in Settings → Notifications. Writes the icon into
/// `data_dir`. No admin rights needed; harmless to repeat.
#[cfg(windows)]
pub fn register_notifications(data_dir: &Path) -> Result<(), PlatformError> {
    let icon = data_dir.join("app-icon.ico");
    std::fs::write(&icon, crate::icon_art::app_icon_ico())?;
    let key = windows_registry::CURRENT_USER
        .create(format!(r"Software\Classes\AppUserModelId\{APP_ID}"))
        .map_err(notify_error)?;
    key.set_string("DisplayName", "WFH Tracker")
        .map_err(notify_error)?;
    key.set_string("IconUri", icon.to_string_lossy())
        .map_err(notify_error)?;
    Ok(())
}

#[cfg(not(windows))]
pub fn register_notifications(_data_dir: &Path) -> Result<(), PlatformError> {
    Ok(())
}

#[cfg(windows)]
fn notify_error(e: impl std::fmt::Display) -> PlatformError {
    PlatformError::Notifications(e.to_string())
}

/// Windows toasts / macOS notification centre.
#[derive(Debug, Default)]
pub struct DesktopNotifier;

impl Notifier for DesktopNotifier {
    fn notify(&self, title: &str, body: &str) {
        let (title, body) = (title.to_owned(), body.to_owned());
        // Showing a toast can block briefly; keep it off the UI thread.
        std::thread::spawn(move || {
            let shown = notify_rust::Notification::new()
                .appname("WFH Tracker")
                .app_id(APP_ID)
                .summary(&title)
                .body(&body)
                .show();
            if let Err(e) = shown {
                eprintln!("wfh-tracker: notification failed: {e}");
            }
        });
    }
}
