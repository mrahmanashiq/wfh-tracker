//! OS adapters for WFH Tracker. Everything here touches the real machine,
//! so it is kept thin and checked by hand; the rules it serves live in `wt-core`.

mod autostart;
mod hotkey;
mod icon_art;
mod notify;
mod tray;
mod updater;

use std::path::PathBuf;

use jiff::Timestamp;
use wt_core::Clock;

pub use autostart::LoginAutostart;
pub use hotkey::MeetingHotkey;
pub use icon_art::app_icon_rgba;
pub use notify::DesktopNotifier;
pub use tray::{Tray, TrayAction};
pub use updater::ReleaseUpdater;

#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    #[error("could not find the user's home folder")]
    NoHomeDir,
    #[error("the app's path is not valid UTF-8")]
    NonUtf8Path,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("start at login: {0}")]
    Autostart(String),
    #[error("tray icon: {0}")]
    Tray(String),
    #[error("meeting hotkey: {0}")]
    Hotkey(String),
    #[error("updates: {0}")]
    Update(String),
}

/// Command-line flag the autostart entry passes, so a login start stays in the tray.
pub const MINIMIZED_FLAG: &str = "--minimized";

/// Wall-clock time from the OS.
#[derive(Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        Timestamp::now()
    }

    fn time_zone(&self) -> jiff::tz::TimeZone {
        jiff::tz::TimeZone::system()
    }
}

/// Reads a font file from the OS font folder (Windows only for now), e.g.
/// `segoeui.ttf`. `None` when it isn't there, so the caller can fall back.
pub fn system_font(file: &str) -> Option<Vec<u8>> {
    let windir = std::env::var_os("WINDIR")?;
    std::fs::read(std::path::Path::new(&windir).join("Fonts").join(file)).ok()
}

/// Per-user data folder, e.g. `%APPDATA%\wfh-tracker\data` on Windows.
/// `WFH_TRACKER_DATA_DIR` overrides it (for trying things on a throwaway copy).
pub fn data_dir() -> Result<PathBuf, PlatformError> {
    if let Some(dir) = std::env::var_os("WFH_TRACKER_DATA_DIR") {
        return Ok(PathBuf::from(dir));
    }
    directories::ProjectDirs::from("", "", "wfh-tracker")
        .map(|dirs| dirs.data_dir().to_path_buf())
        .ok_or(PlatformError::NoHomeDir)
}
