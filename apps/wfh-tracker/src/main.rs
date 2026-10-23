//! Wires the real adapters (SQLite, OS clock, tray, hotkey, notifications,
//! autostart, foreground window) into the UI.

// No console window behind the app in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;
use std::sync::Arc;

use eframe::egui::{self, IconData, ViewportBuilder};
use wt_core::DistractionRepo;
use wt_core::{Autostart, CategoryRepo, SettingsRepo};
use wt_platform::{
    DesktopNotifier, LoginAutostart, MINIMIZED_FLAG, MeetingHotkey, OsForegroundWatcher,
    ReleaseUpdater, SystemClipboard, SystemClock, Tray, TrayAction, app_icon_rgba,
};
use wt_storage::SqliteStore;
use wt_ui_egui::{Services, UiFonts, WfhApp};

/// Where released versions are announced (a signed manifest on GitHub Releases).
const UPDATE_MANIFEST: &str =
    "https://github.com/mrahmanashiq/wfh-tracker/releases/latest/download/latest.json";
/// Public half of the release signing key; downloads must match it.
const UPDATE_PUBLIC_KEY: &str = include_str!("../../../packaging/updater.key.pub");

fn main() -> Result<(), Box<dyn Error>> {
    let start_hidden = std::env::args().any(|a| a == MINIMIZED_FLAG);

    let dir = wt_platform::data_dir()?;
    std::fs::create_dir_all(&dir)?;
    if let Err(e) = wt_platform::register_notifications(&dir) {
        eprintln!("wfh-tracker: {e}");
    }
    let clock = Arc::new(SystemClock);
    let store = SqliteStore::open(&dir.join("wfh-tracker.db"), clock.clone())?;
    store.seed_defaults()?;
    store.seed_default_rules()?;

    // Keep the OS entry in line with the saved preference (on by default).
    // Debug builds leave it alone so `cargo run` never registers target/debug.
    let autostart = LoginAutostart::for_current_exe()?;
    if !cfg!(debug_assertions)
        && let Ok(settings) = store.load()
        && let Err(e) = autostart.set_enabled(settings.autostart)
    {
        eprintln!("wfh-tracker: {e}");
    }

    let size = 64;
    let rgba = app_icon_rgba(size);
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title("WFH Tracker")
            .with_inner_size([980.0, 660.0])
            .with_min_inner_size([760.0, 480.0])
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
                foreground: Box::new(OsForegroundWatcher),
                fonts: segoe_ui(),
                updater: release_updater(),
                clipboard: Box::new(SystemClipboard),
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

/// The updater for installed (release) builds; development builds get none.
fn release_updater() -> Option<Arc<dyn wt_core::Updater>> {
    if cfg!(debug_assertions) {
        return None;
    }
    ReleaseUpdater::new(
        env!("CARGO_PKG_VERSION"),
        UPDATE_MANIFEST,
        UPDATE_PUBLIC_KEY,
    )
    .inspect_err(|e| eprintln!("wfh-tracker: {e}"))
    .ok()
    .map(|u| Arc::new(u) as Arc<dyn wt_core::Updater>)
}

/// Windows' own UI font, for a native feel.
fn segoe_ui() -> Option<UiFonts> {
    Some(UiFonts {
        regular: wt_platform::system_font("segoeui.ttf")?,
        semibold: wt_platform::system_font("seguisb.ttf")?,
        emoji: wt_platform::system_font("seguiemj.ttf"),
    })
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
