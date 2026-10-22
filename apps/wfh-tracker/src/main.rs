//! Wires the real adapters (SQLite, OS clock, tray, hotkey, notifications,
//! autostart) into the UI.

// No console window behind the app in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;
use std::sync::Arc;

use eframe::egui::{self, IconData, ViewportBuilder};
use wt_core::{Autostart, CategoryRepo, SettingsRepo};
use wt_platform::{
    DesktopNotifier, LoginAutostart, MINIMIZED_FLAG, MeetingHotkey, SystemClock, Tray, TrayAction,
    app_icon_rgba,
};
use wt_storage::SqliteStore;
use wt_ui_egui::{Services, WfhApp};

fn main() -> Result<(), Box<dyn Error>> {
    let start_hidden = std::env::args().any(|a| a == MINIMIZED_FLAG);

    let dir = wt_platform::data_dir()?;
    std::fs::create_dir_all(&dir)?;
    let clock = Arc::new(SystemClock);
    let store = SqliteStore::open(&dir.join("wfh-tracker.db"), clock.clone())?;
    store.seed_defaults()?;

    // Keep the OS entry in line with the saved preference (on by default).
    // Debug builds leave it alone so `cargo run` never registers target/debug.
    let autostart = LoginAutostart::for_current_exe()?;
    if !cfg!(debug_assertions)
        && let Ok(settings) = store.load()
        && let Err(e) = autostart.set_enabled(settings.autostart)
    {
        eprintln!("wfh-tracker: {e}");
    }

    let (rgba, size) = app_icon_rgba();
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title("WFH Tracker")
            .with_inner_size([820.0, 600.0])
            .with_min_inner_size([640.0, 440.0])
            .with_icon(IconData {
                rgba,
                width: size,
                height: size,
            }),
        ..Default::default()
    };

    eframe::run_native(
        "WFH Tracker",
        options,
        Box::new(move |cc| {
            let services = Services {
                clock,
                autostart: Box::new(autostart),
                notifier: Box::new(DesktopNotifier),
            };
            let (app, shell) = WfhApp::new(&cc.egui_ctx, store, services, start_hidden);
            // Tray and hotkey are created here, on the event-loop thread, as
            // both libraries require on Windows.
            let hotkey_shell = shell.clone();
            let hotkey = MeetingHotkey::register(move || hotkey_shell.toggle_meeting())
                .inspect_err(|e| eprintln!("wfh-tracker: {e} (the Meeting button still works)"))
                .ok();
            let tray = Tray::create(move |action| match action {
                TrayAction::Open => shell.show(),
                TrayAction::Quit => shell.quit(),
            })?;
            Ok(Box::new(WithOsHooks {
                app,
                _tray: tray,
                _hotkey: hotkey,
            }))
        }),
    )?;
    Ok(())
}

/// Keeps the tray icon and hotkey registered for as long as the app runs.
struct WithOsHooks<A> {
    app: A,
    _tray: Tray,
    _hotkey: Option<MeetingHotkey>,
}

impl<A: eframe::App> eframe::App for WithOsHooks<A> {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.app.logic(ctx, frame);
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.app.ui(ui, frame);
    }
}
