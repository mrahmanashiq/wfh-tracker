//! OS adapters for WFH Tracker. Everything here touches the real machine,
//! so it is kept thin and checked by hand; the rules it serves live in `wt-core`.

mod autostart;
mod tray;

use std::path::PathBuf;

use jiff::Timestamp;
use wt_core::Clock;

pub use autostart::LoginAutostart;
pub use tray::{Tray, TrayAction, app_icon_rgba};

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

/// Per-user data folder, e.g. `%APPDATA%\wfh-tracker\data` on Windows.
pub fn data_dir() -> Result<PathBuf, PlatformError> {
    directories::ProjectDirs::from("", "", "wfh-tracker")
        .map(|dirs| dirs.data_dir().to_path_buf())
        .ok_or(PlatformError::NoHomeDir)
}
