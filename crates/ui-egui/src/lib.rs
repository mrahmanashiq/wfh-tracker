//! egui front end for WFH Tracker. It talks to the rest of the app only
//! through the traits in `wt-core`, so the UI can be replaced on its own.

mod settings_screen;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use egui::{Context, ViewportCommand};
use wt_core::{Autostart, SettingsRepo};

pub use settings_screen::SettingsScreen;

/// Lets other threads (the tray icon) show the window or quit the app.
#[derive(Clone)]
pub struct ShellHandle {
    ctx: Context,
    quitting: Arc<AtomicBool>,
}

impl ShellHandle {
    pub fn show(&self) {
        self.ctx.send_viewport_cmd(ViewportCommand::Visible(true));
        self.ctx
            .send_viewport_cmd(ViewportCommand::Minimized(false));
        self.ctx.send_viewport_cmd(ViewportCommand::Focus);
        self.ctx.request_repaint();
    }

    pub fn quit(&self) {
        self.quitting.store(true, Ordering::SeqCst);
        self.ctx.send_viewport_cmd(ViewportCommand::Close);
        self.ctx.request_repaint();
    }
}

/// The app window. Closing it hides it to the tray; only [`ShellHandle::quit`] exits.
pub struct WfhApp<S> {
    store: S,
    autostart: Box<dyn Autostart>,
    settings: SettingsScreen,
    quitting: Arc<AtomicBool>,
    start: Start,
}

/// eframe always shows the window once its first frame is painted, so a hidden
/// (login) start lets that frame paint, then hides the window on the next pass.
#[derive(PartialEq, Eq)]
enum Start {
    Shown,
    HideAfterFirstFrame,
    HideNow,
}

impl<S: SettingsRepo> WfhApp<S> {
    pub fn new(
        ctx: &Context,
        store: S,
        autostart: Box<dyn Autostart>,
        start_hidden: bool,
    ) -> (Self, ShellHandle) {
        let quitting = Arc::new(AtomicBool::new(false));
        let handle = ShellHandle {
            ctx: ctx.clone(),
            quitting: quitting.clone(),
        };
        let settings = SettingsScreen::load(&store);
        let app = Self {
            store,
            autostart,
            settings,
            quitting,
            start: if start_hidden {
                Start::HideAfterFirstFrame
            } else {
                Start::Shown
            },
        };
        (app, handle)
    }
}

impl<S: SettingsRepo> eframe::App for WfhApp<S> {
    /// Runs even while the window is hidden, so tray commands still land.
    fn logic(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.viewport().close_requested()) && !self.quitting.load(Ordering::SeqCst) {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(ViewportCommand::Visible(false));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        match self.start {
            Start::Shown => {}
            Start::HideAfterFirstFrame => {
                self.start = Start::HideNow;
                ui.ctx().request_repaint();
            }
            Start::HideNow => {
                ui.ctx().send_viewport_cmd(ViewportCommand::Visible(false));
                self.start = Start::Shown;
            }
        }
        egui::Panel::top("top_bar").show(ui, |ui| {
            ui.add_space(4.0);
            ui.heading("WFH Tracker");
            ui.add_space(4.0);
        });
        egui::CentralPanel::default().show(ui, |ui| {
            self.settings.ui(ui, &self.store, self.autostart.as_ref());
        });
    }
}
