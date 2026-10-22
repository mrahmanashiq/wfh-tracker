use egui::{RichText, Ui};
use jiff::{SignedDuration, Timestamp};
use wt_core::repo::RepoResult;
use wt_core::{
    CategoryKind, Clock, MeetingToggle, Notice, Notifier, OfficeStatus, Reminders, Session,
    SessionRepo, Settings, Tick, TimerTarget, office_status, office_time,
};

use crate::widgets::message;

/// A timer left running is stopped once the app has been gone this long.
const RECOVERY_GAP: SignedDuration = SignedDuration::from_mins(3);

/// The always-visible strip: running timer, today's office time, Meeting,
/// breaks, resume and quick start. Also turns reminder ticks into notifications.
#[derive(Default)]
pub struct StatusBar {
    reminders: Reminders,
    /// What a meeting or break interrupted, with a label for the button.
    resume: Option<(TimerTarget, String)>,
    /// Explains a timer stopped by heartbeat recovery.
    banner: Option<String>,
    message: Option<String>,
}

enum Action {
    Stop,
    ToggleMeeting,
    TakeBreak,
    Resume,
    QuickStart(wt_core::Id),
    DismissBanner,
}

impl StatusBar {
    pub fn ui(
        &mut self,
        ui: &mut Ui,
        store: &dyn SessionRepo,
        settings: &Settings,
        clock: &dyn Clock,
    ) {
        let now = clock.now();
        let loaded = (|| -> RepoResult<_> {
            Ok((
                store.running()?,
                store.sessions_since(two_days_before(now))?,
                store.list()?,
            ))
        })();
        let (running, sessions, categories) = match loaded {
            Ok(loaded) => loaded,
            Err(e) => {
                message(ui, &Some(format!("Could not load timers: {e}")));
                return;
            }
        };

        let mut action = None;
        ui.horizontal(|ui| {
            match &running {
                Some(session) => {
                    ui.label(RichText::new(session_label(store, session)).strong());
                    ui.weak(clock_text(now.duration_since(session.started_at)));
                    if ui.button("Stop").clicked() {
                        action = Some(Action::Stop);
                    }
                }
                None => {
                    ui.weak("No timer running");
                }
            }
            ui.separator();

            let today = clock.today();
            let office = office_time(&sessions, today, &clock.time_zone(), now);
            ui.label(office_text(office, settings, today.weekday()));
            ui.separator();

            let in_meeting = running.as_ref().is_some_and(Session::is_meeting);
            let on_break = running.as_ref().is_some_and(Session::is_break);
            let meeting_label = if in_meeting { "End meeting" } else { "Meeting" };
            if ui.button(meeting_label).clicked() {
                action = Some(Action::ToggleMeeting);
            }
            if running.is_some() && !on_break && !in_meeting && ui.button("Take break").clicked() {
                action = Some(Action::TakeBreak);
            }
            if let Some((_, label)) = &self.resume
                && (running.is_none() || on_break)
                && ui.button(format!("Resume {label}")).clicked()
            {
                action = Some(Action::Resume);
            }
        });

        ui.horizontal_wrapped(|ui| {
            ui.weak("Quick start:");
            for c in categories.iter().filter(|c| {
                matches!(
                    c.kind,
                    CategoryKind::Personal | CategoryKind::Learning | CategoryKind::Custom
                )
            }) {
                if ui.small_button(format!("Start {}", c.name)).clicked() {
                    action = Some(Action::QuickStart(c.meta.id));
                }
            }
        });

        if let Some(banner) = &self.banner {
            ui.horizontal(|ui| {
                ui.label(banner);
                if ui.small_button("Dismiss").clicked() {
                    action = Some(Action::DismissBanner);
                }
            });
        }
        message(ui, &self.message);

        if let Some(action) = action {
            self.apply(action, store);
        }
    }

    /// The Meeting button and the global hotkey both land here.
    pub fn toggle_meeting(&mut self, store: &dyn SessionRepo) {
        self.apply(Action::ToggleMeeting, store);
    }

    /// Recovers a timer left running across a gap, then turns due reminders
    /// into desktop notifications. Call it every few seconds, hidden or not.
    pub fn tick(
        &mut self,
        store: &dyn SessionRepo,
        settings: &Settings,
        clock: &dyn Clock,
        notifier: &dyn Notifier,
    ) {
        let now = clock.now();
        let tz = clock.time_zone();
        match store.recover(now, RECOVERY_GAP) {
            Ok(Some(stopped)) => {
                let at = stopped.end_or(now).to_zoned(tz.clone()).strftime("%H:%M");
                self.banner = Some(format!(
                    "Stopped {} at {at} while the app wasn't running.",
                    session_label(store, &stopped)
                ));
            }
            Ok(None) => {}
            Err(e) => self.message = Some(e.to_string()),
        }

        let (sessions, interval) = match (|| -> RepoResult<_> {
            let interval = match store.running()?.and_then(|s| s.task_id) {
                Some(task) => store.break_interval_for(task)?,
                None => None,
            };
            Ok((store.sessions_since(two_days_before(now))?, interval))
        })() {
            Ok(loaded) => loaded,
            Err(e) => {
                self.message = Some(e.to_string());
                return;
            }
        };
        let notices = self.reminders.tick(Tick {
            sessions: &sessions,
            now,
            tz: &tz,
            settings,
            break_interval_min: interval,
        });
        for notice in notices {
            let (title, body) = notice_text(notice, settings, interval);
            notifier.notify(title, &body);
        }
    }

    fn apply(&mut self, action: Action, store: &dyn SessionRepo) {
        let result: RepoResult<()> = match action {
            Action::Stop => store.stop().map(|_| ()),
            Action::ToggleMeeting => store.toggle_meeting().map(|toggle| {
                if let MeetingToggle::Started { paused } = toggle {
                    self.offer_resume(store, paused.as_ref());
                }
            }),
            Action::TakeBreak => store
                .take_break()
                .map(|switch| self.offer_resume(store, switch.ended.as_ref())),
            Action::Resume => match self.resume.take() {
                Some((target, _)) => store.start(target).map(|_| ()),
                None => Ok(()),
            },
            Action::QuickStart(id) => {
                self.resume = None;
                store.start(TimerTarget::Category(id)).map(|_| ())
            }
            Action::DismissBanner => {
                self.banner = None;
                Ok(())
            }
        };
        self.message = result.err().map(|e| e.to_string());
    }

    /// Offers to resume what a meeting or break interrupted. Interrupting
    /// nothing clears the offer; interrupting a break or meeting keeps it.
    fn offer_resume(&mut self, store: &dyn SessionRepo, paused: Option<&Session>) {
        match paused {
            None => self.resume = None,
            Some(s) if s.is_break() || s.is_meeting() => {}
            Some(work) => {
                self.resume = Some((TimerTarget::resuming(work), session_label(store, work)));
            }
        }
    }
}

/// The task title, or the category name for a task-less session.
fn session_label(store: &dyn SessionRepo, session: &Session) -> String {
    let task = session.task_id.and_then(|id| store.get(id).ok());
    match task {
        Some(task) => task.title,
        None => store
            .list()
            .ok()
            .and_then(|cats| cats.into_iter().find(|c| c.meta.id == session.category_id))
            .map_or_else(|| "Timer".to_owned(), |c| c.name),
    }
}

fn two_days_before(now: Timestamp) -> Timestamp {
    now.checked_sub(SignedDuration::from_hours(48))
        .unwrap_or(now)
}

/// `1h 30m`
fn hours_minutes(d: SignedDuration) -> String {
    let m = d.as_mins().max(0);
    format!("{}h {:02}m", m / 60, m % 60)
}

/// `8h`, or `7h 30m` when the target isn't whole hours.
fn target_text(minutes: u32) -> String {
    match minutes % 60 {
        0 => format!("{}h", minutes / 60),
        m => format!("{}h {m:02}m", minutes / 60),
    }
}

/// `00:42:13`
fn clock_text(d: SignedDuration) -> String {
    let s = d.as_secs().max(0);
    format!("{:02}:{:02}:{:02}", s / 3600, s % 3600 / 60, s % 60)
}

fn office_text(
    office: SignedDuration,
    settings: &Settings,
    weekday: jiff::civil::Weekday,
) -> String {
    let worked = hours_minutes(office);
    if !settings.workdays.contains(&weekday) {
        return if office.is_positive() {
            format!("Office {worked} · day off, {worked} overtime")
        } else {
            format!("Office {worked} · day off")
        };
    }
    let target = target_text(settings.office_target_min);
    match office_status(office, settings, weekday) {
        OfficeStatus::Overtime { minutes } => {
            format!("Office {worked} / {target} · {minutes}m overtime")
        }
        _ => format!("Office {worked} / {target}"),
    }
}

fn notice_text(
    notice: Notice,
    settings: &Settings,
    interval: Option<u32>,
) -> (&'static str, String) {
    match notice {
        Notice::TargetReached => (
            "Office day complete",
            format!(
                "{} of office work today. Time to wrap up.",
                target_text(settings.office_target_min)
            ),
        ),
        Notice::SoftCeiling => (
            "Past your office limit",
            format!(
                "{} of office work today. Anything more is overtime.",
                target_text(settings.office_ceiling_min)
            ),
        ),
        Notice::BreakDue => (
            "Time for a break",
            format!(
                "You've been working for {} min. Take {}.",
                interval.unwrap_or(settings.break_interval_min),
                settings.break_length_min
            ),
        ),
        Notice::BreakOver => ("Break over", "Ready to get back to it?".to_owned()),
    }
}
