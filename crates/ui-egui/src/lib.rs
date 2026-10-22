//! egui front end for WFH Tracker. It talks to the rest of the app only
//! through the traits in `wt-core`, so the UI can be replaced on its own.

mod categories_screen;
mod settings_screen;
mod tasks_screen;
mod widgets;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use egui::{Context, ViewportCommand};
use wt_core::{Autostart, CategoryRepo, Clock, SettingsRepo, TaskRepo};

pub use categories_screen::CategoriesScreen;
pub use settings_screen::SettingsScreen;
pub use tasks_screen::TasksScreen;

/// Anything that stores everything the UI needs.
pub trait Store: SettingsRepo + TaskRepo + CategoryRepo {}
impl<T: SettingsRepo + TaskRepo + CategoryRepo> Store for T {}

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
    clock: Arc<dyn Clock>,
    autostart: Box<dyn Autostart>,
    screen: Screen,
    tasks: TasksScreen,
    categories: CategoriesScreen,
    settings: SettingsScreen,
    quitting: Arc<AtomicBool>,
    start: Start,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    Tasks,
    Categories,
    Settings,
}

/// eframe always shows the window once its first frame is painted, so a hidden
/// (login) start lets that frame paint, then hides the window on the next pass.
#[derive(PartialEq, Eq)]
enum Start {
    Shown,
    HideAfterFirstFrame,
    HideNow,
}

impl<S: Store> WfhApp<S> {
    pub fn new(
        ctx: &Context,
        store: S,
        clock: Arc<dyn Clock>,
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
            clock,
            autostart,
            screen: Screen::Tasks,
            tasks: TasksScreen::default(),
            categories: CategoriesScreen::default(),
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

impl<S: Store> eframe::App for WfhApp<S> {
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
        egui::Panel::left("nav")
            .resizable(false)
            .default_size(140.0)
            .show(ui, |ui| {
                ui.add_space(8.0);
                for (screen, name) in [
                    (Screen::Tasks, "Tasks"),
                    (Screen::Categories, "Categories"),
                    (Screen::Settings, "Settings"),
                ] {
                    ui.selectable_value(&mut self.screen, screen, name);
                }
            });
        egui::CentralPanel::default().show(ui, |ui| match self.screen {
            Screen::Tasks => {
                self.tasks
                    .ui(ui, &self.store, &self.store, self.clock.as_ref());
            }
            Screen::Categories => self.categories.ui(ui, &self.store),
            Screen::Settings => self.settings.ui(ui, &self.store, self.autostart.as_ref()),
        });
    }
}
