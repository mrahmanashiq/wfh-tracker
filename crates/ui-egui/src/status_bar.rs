use egui::{Color32, CornerRadius, RichText, Sense, Ui, vec2};
use jiff::{SignedDuration, Timestamp};
use wt_core::clock_text as time_of_day;
use wt_core::repo::RepoResult;
use wt_core::{
    Block, CategoryKind, Clock, MeetingToggle, Notice, Notifier, OfficeStatus, Reminders, Session,
    SessionRepo, Settings, Tick, TimerTarget, active_block, hours_text, office_status, office_time,
    target_text, tracked_time,
};

use crate::theme::{palette, radius, size, space};
use crate::widgets::{
    Icon, color32, icon_button, message, pill, progress_bar, section_label, sidebar_row,
};

/// The office-time bar in the header.
const OFFICE_BAR_WIDTH: f32 = 150.0;

/// A timer left running is stopped once the app has been gone this long.
const RECOVERY_GAP: SignedDuration = SignedDuration::from_mins(3);

/// The header strip: running timer, today's office time, Meeting, breaks and
/// resume; plus the sidebar's quick-start list. Also turns reminder ticks into
/// notifications.
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
        blocks: &[Block],
    ) {
        let now = clock.now();
        let loaded = (|| -> RepoResult<_> {
            Ok((
                store.running()?,
                store.sessions_since(two_days_before(now))?,
            ))
        })();
        let (running, sessions) = match loaded {
            Ok(loaded) => loaded,
            Err(e) => {
                message(ui, &Some(format!("Could not load timers: {e}")));
                return;
            }
        };

        let p = palette(ui);
        let mut action = None;
        ui.horizontal(|ui| {
            ui.set_min_height(size::ROW);
            match &running {
                Some(session) => {
                    let color = timer_color(store, session, p.accent);
                    egui::Frame::new()
                        .fill(p.accent_soft)
                        .corner_radius(CornerRadius::same(radius::PILL + 2))
                        .inner_margin(egui::Margin::symmetric(space::MD as i8, 3))
                        .show(ui, |ui| {
                            let (dot, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                            ui.painter().circle_filled(dot.center(), 4.0, color);
                            ui.label(
                                RichText::new(session_label(store, session))
                                    .color(p.text)
                                    .strong(),
                            );
                            ui.label(
                                RichText::new(clock_text(tracked_time(
                                    &sessions,
                                    TimerTarget::resuming(session),
                                    clock.today(),
                                    &clock.time_zone(),
                                    now,
                                )))
                                .color(p.accent)
                                .monospace(),
                            );
                            if icon_button(ui, Icon::Stop, "Stop", p.accent).clicked() {
                                action = Some(Action::Stop);
                            }
                        });
                }
                None => {
                    ui.label(RichText::new("No timer running").color(p.secondary));
                }
            }

            ui.add_space(space::MD);
            if let Some(block) = active_block(blocks, now, &clock.time_zone()) {
                ui.label(
                    RichText::new(format!(
                        "{} block · until {}",
                        block_name(store, block),
                        time_of_day(block.end_min)
                    ))
                    .small()
                    .color(p.accent),
                );
                ui.add_space(space::SM);
            }
            let today = clock.today();
            let office = office_time(&sessions, today, &clock.time_zone(), now);
            let status = office_status(office, settings, today.weekday());
            // Fixed width, so the pills on the right never cover it.
            ui.allocate_ui(vec2(OFFICE_BAR_WIDTH + space::XL * 2.0, size::ROW), |ui| {
                ui.spacing_mut().item_spacing.y = space::XS;
                ui.label(
                    RichText::new(office_text(office, settings, today.weekday()))
                        .small()
                        .color(p.secondary),
                );
                let over = matches!(status, OfficeStatus::Overtime { .. });
                office_bar(
                    ui,
                    office,
                    settings.office_target_min,
                    if over { p.danger } else { p.accent },
                );
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let in_meeting = running.as_ref().is_some_and(Session::is_meeting);
                let on_break = running.as_ref().is_some_and(Session::is_break);
                let (label, fill, color) = if in_meeting {
                    ("End meeting", p.danger, Color32::WHITE)
                } else {
                    ("Meeting", p.hover, p.text)
                };
                if pill(ui, label, fill, color).clicked() {
                    action = Some(Action::ToggleMeeting);
                }
                if running.is_some()
                    && !on_break
                    && !in_meeting
                    && pill(ui, "Take break", p.hover, p.text).clicked()
                {
                    action = Some(Action::TakeBreak);
                }
                if let Some((_, label)) = &self.resume
                    && (running.is_none() || on_break)
                    && pill(ui, &format!("Resume {label}"), p.accent, Color32::WHITE).clicked()
                {
                    action = Some(Action::Resume);
                }
            });
        });

        if let Some(banner) = &self.banner {
            ui.horizontal(|ui| {
                ui.label(RichText::new(banner).color(p.secondary));
                if ui.add(egui::Button::new("Dismiss").frame(false)).clicked() {
                    action = Some(Action::DismissBanner);
                }
            });
        }
        message(ui, &self.message);

        if let Some(action) = action {
            self.apply(action, store);
        }
    }

    /// The sidebar's "Track time" list: one row per personal, learning or
    /// custom category; a click starts its timer.
    pub fn quick_start(&mut self, ui: &mut Ui, store: &dyn SessionRepo) {
        let Ok(categories) = store.list() else {
            return;
        };
        section_label(ui, "Track time");
        let mut action = None;
        for c in categories.iter().filter(|c| {
            matches!(
                c.kind,
                CategoryKind::Personal | CategoryKind::Learning | CategoryKind::Custom
            )
        }) {
            let color = color32(c.color);
            let row = sidebar_row(
                ui,
                false,
                |painter, rect| {
                    painter.circle_filled(rect.center(), 4.5, color);
                },
                &c.name,
                None,
                &format!("Start {}", c.name),
            );
            if row.clicked() {
                action = Some(Action::QuickStart(c.meta.id));
            }
        }
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
        blocks: &[Block],
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
            blocks,
        });
        for notice in notices {
            let (title, body) = notice_text(notice, settings, interval, store, blocks);
            notifier.notify(&title, &body);
        }
    }

    fn apply(&mut self, action: Action, store: &dyn SessionRepo) {
        let result: RepoResult<()> = match action {
            Action::Stop => store.stop().map(|_| ()),
            Action::ToggleMeeting => store.toggle_meeting().map(|toggle| match toggle {
                MeetingToggle::Started { paused } => self.offer_resume(store, paused.as_ref()),
                // The interrupted work restarted by itself; nothing left to offer.
                MeetingToggle::Ended { .. } => self.resume = None,
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
    let worked = hours_text(office);
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

/// The block's label, or its category's name.
fn block_name(store: &dyn SessionRepo, block: &Block) -> String {
    if !block.label.is_empty() {
        return block.label.clone();
    }
    store
        .list()
        .ok()
        .and_then(|cats| cats.into_iter().find(|c| c.meta.id == block.category_id))
        .map_or_else(|| "Scheduled".to_owned(), |c| c.name)
}

fn notice_text(
    notice: Notice,
    settings: &Settings,
    interval: Option<u32>,
    store: &dyn SessionRepo,
    blocks: &[Block],
) -> (String, String) {
    let (title, body) = match notice {
        Notice::BlockStarted { block } => {
            let title = blocks.iter().find(|b| b.meta.id == block).map_or_else(
                || "Scheduled block".to_owned(),
                |b| format!("{} block {}", block_name(store, b), b.times_text()),
            );
            return (title, "Time to start your timer.".to_owned());
        }
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
    };
    (title.to_owned(), body)
}

/// A thin bar: office time against the daily target (full when reached).
fn office_bar(ui: &mut Ui, office: SignedDuration, target_min: u32, color: Color32) {
    let share = office.as_mins() as f32 / target_min.max(1) as f32;
    progress_bar(ui, OFFICE_BAR_WIDTH, share, color);
}

/// The running session's category colour, falling back to `default`.
fn timer_color(store: &dyn SessionRepo, session: &Session, default: Color32) -> Color32 {
    store
        .list()
        .ok()
        .and_then(|cats| cats.into_iter().find(|c| c.meta.id == session.category_id))
        .map_or(default, |c| color32(c.color))
}
