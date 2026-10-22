use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use egui::{Color32, Context, RichText, Ui};
use wt_core::Updater;

use crate::theme::palette;
use crate::widgets::{pill, text_button};

/// How often the app looks for a new version on its own.
const CHECK_EVERY: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Clone, Debug, PartialEq, Eq)]
enum State {
    Idle,
    Checking,
    UpToDate,
    Downloading(String),
    /// Downloaded and verified; waiting for the user to restart.
    Ready(String),
    Failed(String),
}

/// Checks for updates daily in the background, downloads them, and offers a
/// restart. Never restarts on its own.
pub struct UpdateController {
    updater: Option<Arc<dyn Updater>>,
    state: Arc<Mutex<State>>,
    last_check: Option<Instant>,
    worker: Option<JoinHandle<()>>,
}

impl UpdateController {
    /// `None` turns updates off (development builds).
    pub fn new(updater: Option<Arc<dyn Updater>>) -> Self {
        Self {
            updater,
            state: Arc::new(Mutex::new(State::Idle)),
            last_check: None,
            worker: None,
        }
    }

    /// Starts the daily check when it's due. Call it regularly.
    pub fn tick(&mut self, ctx: &Context) {
        if self.last_check.is_none_or(|t| t.elapsed() >= CHECK_EVERY) {
            self.check_now(ctx);
        }
    }

    /// Checks and, if there's something new, downloads it, off the UI thread.
    pub fn check_now(&mut self, ctx: &Context) {
        let Some(updater) = self.updater.clone() else {
            return;
        };
        if self.worker.as_ref().is_some_and(|w| !w.is_finished()) {
            return;
        }
        self.last_check = Some(Instant::now());
        let state = self.state.clone();
        let ctx = ctx.clone();
        set(&state, State::Checking);
        self.worker = Some(std::thread::spawn(move || {
            let next = match updater.check() {
                Ok(None) => State::UpToDate,
                Ok(Some(update)) => {
                    set(&state, State::Downloading(update.version.clone()));
                    ctx.request_repaint();
                    match updater.download() {
                        Ok(()) => State::Ready(update.version),
                        Err(e) => State::Failed(format!("Couldn't download the update: {e}")),
                    }
                }
                Err(e) => State::Failed(format!("Couldn't check for updates: {e}")),
            };
            set(&state, next);
            ctx.request_repaint();
        }));
    }

    /// Waits for a running check to finish. For tests.
    pub fn finish_background_work(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }

    /// The header's "Restart to update" pill, shown once an update is ready.
    pub fn header_ui(&mut self, ui: &mut Ui) {
        if let State::Ready(version) = self.state() {
            let p = palette(ui);
            let label = format!("Restart to update to {version}");
            let restart = pill(ui, &label, p.accent, Color32::WHITE)
                .on_hover_text("Your running timer keeps going across the restart.");
            if restart.clicked() {
                self.install();
            }
        }
    }

    /// The Settings section: version, a manual check, and the current state.
    pub fn settings_ui(&mut self, ui: &mut Ui) {
        let p = palette(ui);
        ui.add_space(20.0);
        ui.label(RichText::new("Updates").strong());
        let Some(updater) = self.updater.clone() else {
            ui.label(RichText::new("Updates are off in development builds.").color(p.secondary));
            return;
        };
        ui.horizontal(|ui| {
            ui.label(format!("Version {}", updater.current_version()));
            let busy = matches!(self.state(), State::Checking | State::Downloading(_));
            if !busy && text_button(ui, "Check for updates", true).clicked() {
                self.check_now(ui.ctx());
            }
        });
        let (text, color) = match self.state() {
            State::Idle => return,
            State::Checking => ("Checking…".to_owned(), p.secondary),
            State::UpToDate => ("You're up to date.".to_owned(), p.secondary),
            State::Downloading(v) => (format!("Downloading {v}…"), p.secondary),
            State::Ready(v) => (
                format!("Version {v} is ready. Restart to finish updating."),
                p.accent,
            ),
            State::Failed(e) => (e, p.danger),
        };
        ui.label(RichText::new(text).color(color));
    }

    fn install(&mut self) {
        if let Some(updater) = &self.updater
            && let Err(e) = updater.install_and_relaunch()
        {
            set(
                &self.state,
                State::Failed(format!("Couldn't install the update: {e}")),
            );
        }
    }

    fn state(&self) -> State {
        self.state.lock().map_or(State::Idle, |s| s.clone())
    }
}

fn set(state: &Mutex<State>, next: State) {
    if let Ok(mut s) = state.lock() {
        *s = next;
    }
}
