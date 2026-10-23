use wt_core::{ForegroundWatcher, ForegroundWindow};

/// The OS's foreground window (active-win-pos-rs). The title and process
/// name are handed to the core matcher and never stored or logged here.
#[derive(Debug, Default)]
pub struct OsForegroundWatcher;

impl ForegroundWatcher for OsForegroundWatcher {
    fn foreground(&self) -> Option<ForegroundWindow> {
        let window = active_win_pos_rs::get_active_window().ok()?;
        // The executable's file name (`chrome.exe`) is what users see in
        // Task Manager; fall back to the app name the OS reports.
        let process = window
            .process_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| !name.is_empty())
            .unwrap_or(window.app_name);
        Some(ForegroundWindow {
            title: window.title,
            process,
        })
    }
}
