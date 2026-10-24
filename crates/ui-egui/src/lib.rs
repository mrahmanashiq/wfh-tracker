//! egui front end for WFH Tracker. It talks to the rest of the app only
//! through the traits in `wt-core`, so the UI can be replaced on its own.

mod cache;
mod categories_screen;
mod dashboard_screen;
mod focus_screen;
mod nudges;
mod planning_screen;
mod reports_screen;
mod settings_screen;
mod status_bar;
mod tasks_screen;
mod theme;
mod updates;
mod widgets;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use egui::{Context, ViewportCommand};
use wt_core::{
    Autostart, BlockRepo, Clock, DistractionEventRepo, DistractionRuleRepo, ForegroundWatcher,
    Notifier, SessionRepo, SettingsRepo, TaskRepo,
};

use crate::theme::{size, space};
use crate::widgets::Icon;

pub use cache::CachedStore;
pub use categories_screen::CategoriesScreen;
pub use dashboard_screen::DashboardScreen;
pub use focus_screen::FocusScreen;
pub use nudges::Nudges;
pub use planning_screen::PlanningScreen;
pub use reports_screen::ReportsScreen;
pub use settings_screen::SettingsScreen;
pub use status_bar::StatusBar;
pub use tasks_screen::TasksScreen;
pub use theme::UiFonts;
pub use updates::UpdateController;

/// How often reminders and the heartbeat are checked, shown or hidden.
const TICK_EVERY: Duration = Duration::from_secs(15);
/// How often the foreground window is checked for distractions.
const POLL_EVERY: Duration = Duration::from_secs(5);

/// Anything that stores everything the UI needs.
pub trait Store: SettingsRepo + PlanStore + DistractionRuleRepo + DistractionEventRepo {}
impl<T: SettingsRepo + PlanStore + DistractionRuleRepo + DistractionEventRepo> Store for T {}

/// What the nudges need: the running timer and blocks (for the context),
/// categories, rules and events.
pub trait NudgeStore: SessionRepo + BlockRepo + DistractionRuleRepo + DistractionEventRepo {}
impl<T: SessionRepo + BlockRepo + DistractionRuleRepo + DistractionEventRepo> NudgeStore for T {}

/// What the reports need: sessions, tasks, categories and distractions.
pub trait ReportStore: SessionRepo + DistractionRuleRepo + DistractionEventRepo {}
impl<T: SessionRepo + DistractionRuleRepo + DistractionEventRepo> ReportStore for T {}

/// What the dashboard needs: sessions, tasks, categories, targets and distractions.
pub trait DashboardStore:
    SessionRepo + wt_core::TargetRepo + DistractionRuleRepo + DistractionEventRepo
{
}
impl<T: SessionRepo + wt_core::TargetRepo + DistractionRuleRepo + DistractionEventRepo>
    DashboardStore for T
{
}

/// What the planning screen needs: sessions (for progress), targets, blocks.
pub trait PlanStore: SessionRepo + wt_core::TargetRepo + wt_core::BlockRepo {}
impl<T: SessionRepo + wt_core::TargetRepo + wt_core::BlockRepo> PlanStore for T {}

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
    /// Every read and write goes through the cache, so frames don't query.
    store: CachedStore<S>,
    /// The minute (since the epoch) the cache was last refreshed for.
    minute: Option<i64>,
    clock: Arc<dyn Clock>,
    autostart: Box<dyn Autostart>,
    notifier: Box<dyn Notifier>,
    foreground: Box<dyn ForegroundWatcher>,
    commands: Receiver<ShellCommand>,
    last_tick: Option<Instant>,
    last_poll: Option<Instant>,
    nudges: Nudges,
    /// The last failed distraction check, shown on the Focus screen.
    nudge_error: Option<String>,
    focus: FocusScreen,
    screen: Screen,
    status: StatusBar,
    tasks: TasksScreen,
    dashboard: DashboardScreen,
    categories: CategoriesScreen,
    reports: ReportsScreen,
    planning: PlanningScreen,
    clipboard: Box<dyn wt_core::Clipboard>,
    settings: SettingsScreen,
    updates: UpdateController,
    quitting: Arc<AtomicBool>,
    start: Start,
    /// The appearance last applied, so it is only set when it changes.
    appearance: Option<wt_core::Appearance>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    Tasks,
    Dashboard,
    Planning,
    Focus,
    Reports,
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
    /// Reads the window in front, for distraction nudges.
    pub foreground: Box<dyn ForegroundWatcher>,
    /// Fonts to use instead of egui's built-in one, e.g. Segoe UI.
    pub fonts: Option<UiFonts>,
    /// `None` in development builds.
    pub updater: Option<Arc<dyn wt_core::Updater>>,
    pub clipboard: Box<dyn wt_core::Clipboard>,
}

impl<S: Store> WfhApp<S> {
    pub fn new(
        ctx: &Context,
        store: S,
        services: Services,
        start_hidden: bool,
    ) -> (Self, ShellHandle) {
        theme::install(ctx, services.fonts);
        let quitting = Arc::new(AtomicBool::new(false));
        let (sender, commands) = channel();
        let handle = ShellHandle {
            ctx: ctx.clone(),
            quitting: quitting.clone(),
            commands: sender,
        };
        let settings = SettingsScreen::load(&store);
        let app = Self {
            store: CachedStore::new(store),
            minute: None,
            clock: services.clock,
            autostart: services.autostart,
            notifier: services.notifier,
            foreground: services.foreground,
            commands,
            last_tick: None,
            last_poll: None,
            nudges: Nudges::default(),
            nudge_error: None,
            focus: FocusScreen::default(),
            screen: Screen::Tasks,
            status: StatusBar::default(),
            tasks: TasksScreen::default(),
            dashboard: DashboardScreen,
            categories: CategoriesScreen::default(),
            reports: ReportsScreen::default(),
            planning: PlanningScreen::default(),
            clipboard: services.clipboard,
            settings,
            updates: UpdateController::new(services.updater),
            quitting,
            start: if start_hidden {
                Start::HideAfterFirstFrame
            } else {
                Start::Shown
            },
            appearance: None,
        };
        (app, handle)
    }
}

impl<S: Store> eframe::App for WfhApp<S> {
    /// Runs even while the window is hidden, so the tray, the hotkey and
    /// reminders keep working from the tray.
    fn logic(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.quitting.load(Ordering::SeqCst) {
                // Quitting stops the watching: close an open distraction now.
                // If this fails, one left open is ended at the last heartbeat next start.
                if let Err(e) = self.nudges.stop(&self.store) {
                    eprintln!("wfh-tracker: could not end the open distraction: {e}");
                }
            } else {
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
                ctx.send_viewport_cmd(ViewportCommand::Visible(false));
            }
        }
        // Reload once a minute too, as a safety net for anything that
        // changes with time alone (writes already refresh the cache).
        let minute = self.clock.now().as_second().div_euclid(60);
        if self.minute != Some(minute) {
            self.minute = Some(minute);
            self.store.invalidate();
        }
        while let Ok(ShellCommand::ToggleMeeting) = self.commands.try_recv() {
            self.status.toggle_meeting(&self.store);
        }
        // Before the reminder tick, so the first poll still sees the last
        // run's heartbeat (to end a distraction a crash left open).
        if self.last_poll.is_none_or(|t| t.elapsed() >= POLL_EVERY) {
            self.last_poll = Some(Instant::now());
            let settings = self.store.load().unwrap_or_default();
            self.nudge_error = self
                .nudges
                .poll(
                    &self.store,
                    &settings,
                    self.clock.as_ref(),
                    self.foreground.as_ref(),
                    self.notifier.as_ref(),
                )
                .err()
                .map(|e| e.to_string());
        }
        if self.last_tick.is_none_or(|t| t.elapsed() >= TICK_EVERY) {
            self.last_tick = Some(Instant::now());
            let settings = self.store.load().unwrap_or_default();
            let blocks = wt_core::BlockRepo::blocks(&self.store).unwrap_or_default();
            self.status.tick(
                &self.store,
                &settings,
                self.clock.as_ref(),
                self.notifier.as_ref(),
                &blocks,
            );
        }
        self.updates.tick(ctx);
        ctx.request_repaint_after(POLL_EVERY);
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
        if self.appearance != Some(settings.appearance) {
            self.appearance = Some(settings.appearance);
            theme::apply(ui.ctx(), settings.appearance);
        }
        let blocks = wt_core::BlockRepo::blocks(&self.store).unwrap_or_default();
        let p = theme::palette(ui);
        egui::Panel::left("nav")
            .resizable(false)
            .exact_size(size::SIDEBAR_WIDTH)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(p.sidebar)
                    .inner_margin(egui::Margin::symmetric(space::MD as i8, space::LG as i8)),
            )
            .show(ui, |ui| self.sidebar(ui));
        egui::Panel::top("status")
            .show_separator_line(true)
            .frame(
                egui::Frame::new()
                    .fill(p.canvas)
                    .inner_margin(egui::Margin::symmetric(space::XL as i8, space::SM as i8)),
            )
            .show(ui, |ui| {
                self.status
                    .ui(ui, &self.store, &settings, self.clock.as_ref(), &blocks);
                self.updates.header_ui(ui);
            });
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(p.canvas)
                    .inner_margin(egui::Margin::symmetric(space::XXL as i8, space::XL as i8)),
            )
            .show(ui, |ui| match self.screen {
                Screen::Tasks => self.tasks.ui(ui, &self.store, self.clock.as_ref()),
                Screen::Dashboard => {
                    self.dashboard
                        .ui(ui, &self.store, &settings, self.clock.as_ref());
                }
                Screen::Planning => {
                    self.planning
                        .ui(ui, &self.store, &settings, self.clock.as_ref());
                }
                Screen::Focus => {
                    self.focus.ui(ui, &self.store, self.nudge_error.as_deref());
                }
                Screen::Reports => self.reports.ui(
                    ui,
                    &self.store,
                    &settings,
                    self.clock.as_ref(),
                    self.clipboard.as_ref(),
                ),
                Screen::Categories => self.categories.ui(ui, &self.store),
                Screen::Settings => {
                    self.settings.ui(ui, &self.store, self.autostart.as_ref());
                    self.updates.settings_ui(ui);
                }
            });
    }
}

impl<S: Store> WfhApp<S> {
    /// Things-style navigation: Today with its open count, Dashboard, Categories,
    /// Settings, then the quick-start list.
    fn sidebar(&mut self, ui: &mut egui::Ui) {
        let p = theme::palette(ui);
        let open = self
            .store
            .list_live()
            .map(|tasks| {
                tasks
                    .iter()
                    .filter(|t| t.parent_id.is_none() && !t.is_done())
                    .count()
            })
            .unwrap_or(0);
        let items = [
            (Screen::Tasks, Icon::Star, p.star, "Today", Some(open)),
            (Screen::Dashboard, Icon::Chart, p.accent, "Dashboard", None),
            (Screen::Planning, Icon::Target, p.accent, "Planning", None),
            (Screen::Focus, Icon::Focus, p.accent, "Focus", None),
            (Screen::Reports, Icon::Doc, p.accent, "Reports", None),
            (Screen::Categories, Icon::Tag, p.accent, "Categories", None),
            (Screen::Settings, Icon::Gear, p.secondary, "Settings", None),
        ];
        for (screen, icon, color, name, count) in items {
            let row = widgets::sidebar_row(
                ui,
                self.screen == screen,
                |painter, rect| icon.paint(painter, rect, color),
                name,
                count,
                name,
            );
            if row.clicked() && self.screen != screen {
                self.screen = screen;
                self.store.invalidate();
            }
        }
        self.status.quick_start(ui, &self.store);
    }
}
