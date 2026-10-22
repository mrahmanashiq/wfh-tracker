//! egui front end for WFH Tracker. It talks to the rest of the app only
//! through the traits in `wt-core`, so the UI can be replaced on its own.

mod categories_screen;
mod settings_screen;
mod status_bar;
mod tasks_screen;
mod widgets;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use egui::{Context, ViewportCommand};
use wt_core::{Autostart, Clock, Notifier, SessionRepo, SettingsRepo};

pub use categories_screen::CategoriesScreen;
pub use settings_screen::SettingsScreen;
pub use status_bar::StatusBar;
pub use tasks_screen::TasksScreen;

/// How often reminders and the heartbeat are checked, shown or hidden.
const TICK_EVERY: Duration = Duration::from_secs(15);

/// Anything that stores everything the UI needs.
pub trait Store: SettingsRepo + SessionRepo {}
impl<T: SettingsRepo + SessionRepo> Store for T {}

/// Requests from outside the UI thread (tray, global hotkey).
enum ShellCommand {
    ToggleMeeting,
}

/// Lets other threads (the tray icon, the hotkey) drive the app.
#[derive(Clone)]
pub struct ShellHandle {
    ctx: Context,
    quitting: Arc<AtomicBool>,
    commands: Sender<ShellCommand>,
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

    /// Same as the Meeting button; works while the window is hidden.
    pub fn toggle_meeting(&self) {
        if self.commands.send(ShellCommand::ToggleMeeting).is_ok() {
            self.ctx.request_repaint();
        }
    }
}

/// The app window. Closing it hides it to the tray; only [`ShellHandle::quit`] exits.
pub struct WfhApp<S> {
    store: S,
    clock: Arc<dyn Clock>,
    autostart: Box<dyn Autostart>,
    notifier: Box<dyn Notifier>,
    commands: Receiver<ShellCommand>,
    last_tick: Option<Instant>,
    screen: Screen,
    status: StatusBar,
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

/// The OS services the window needs, bundled to keep `WfhApp::new` readable.
pub struct Services {
    pub clock: Arc<dyn Clock>,
    pub autostart: Box<dyn Autostart>,
    pub notifier: Box<dyn Notifier>,
}

impl<S: Store> WfhApp<S> {
    pub fn new(
        ctx: &Context,
        store: S,
        services: Services,
        start_hidden: bool,
    ) -> (Self, ShellHandle) {
        let quitting = Arc::new(AtomicBool::new(false));
        let (sender, commands) = channel();
        let handle = ShellHandle {
            ctx: ctx.clone(),
            quitting: quitting.clone(),
            commands: sender,
        };
        let settings = SettingsScreen::load(&store);
        let app = Self {
            store,
            clock: services.clock,
            autostart: services.autostart,
            notifier: services.notifier,
            commands,
            last_tick: None,
            screen: Screen::Tasks,
            status: StatusBar::default(),
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
    /// Runs even while the window is hidden, so the tray, the hotkey and
    /// reminders keep working from the tray.
    fn logic(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.viewport().close_requested()) && !self.quitting.load(Ordering::SeqCst) {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(ViewportCommand::Visible(false));
        }
        while let Ok(ShellCommand::ToggleMeeting) = self.commands.try_recv() {
            self.status.toggle_meeting(&self.store);
        }
        if self.last_tick.is_none_or(|t| t.elapsed() >= TICK_EVERY) {
            self.last_tick = Some(Instant::now());
            let settings = self.store.load().unwrap_or_default();
            self.status.tick(
                &self.store,
                &settings,
                self.clock.as_ref(),
                self.notifier.as_ref(),
            );
        }
        ctx.request_repaint_after(TICK_EVERY);
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
        // The running timer's clock ticks once a second while visible.
        ui.ctx().request_repaint_after(Duration::from_secs(1));

        let settings = self.store.load().unwrap_or_default();
        egui::Panel::top("status").show(ui, |ui| {
            ui.add_space(4.0);
            self.status
                .ui(ui, &self.store, &settings, self.clock.as_ref());
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
            Screen::Tasks => self.tasks.ui(ui, &self.store, self.clock.as_ref()),
            Screen::Categories => self.categories.ui(ui, &self.store),
            Screen::Settings => self.settings.ui(ui, &self.store, self.autostart.as_ref()),
        });
    }
}
