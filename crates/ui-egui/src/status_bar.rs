use egui::{Align, Color32, Layout, Rect, RichText, Sense, Ui, UiBuilder, pos2, vec2};
use jiff::{SignedDuration, Timestamp};
use wt_core::clock_text as time_of_day;
use wt_core::repo::RepoResult;
use wt_core::{
    Block, CallAction, CategoryKind, Clock, MeetingToggle, Notice, Notifier, OfficeStatus,
    Reminders, Session, SessionRepo, Settings, Tick, TimerTarget, active_block, hours_text,
    office_status, office_time, target_text, tracked_time,
};

use crate::theme::{Palette, palette, size, space};
use crate::widgets::{
    HeaderLook, Icon, color32, header_button, hero_time, hero_time_width, message, office_meter,
    running_title, section_label, sidebar_row, stop_button,
};

/// The quiet line under "Nothing running".
const IDLE_HINT: &str = "Start a to-do, or pick one under Track time";

/// Office time this close to the soft ceiling shows as a warning.
const CEILING_NEAR: SignedDuration = SignedDuration::from_mins(30);

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
        let today = clock.today();
        let tz = clock.time_zone();
        let in_meeting = running.as_ref().is_some_and(Session::is_meeting);
        let on_break = running.as_ref().is_some_and(Session::is_break);
        let compact = ui.available_width() < size::HEADER_COMPACT;
        let (strip, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), size::HEADER), Sense::hover());

        // Right to left: Meeting, then Take break or Resume, then the office
        // meter. Whatever width they take, the running block gets the rest.
        let mut right = ui.new_child(
            UiBuilder::new()
                .max_rect(strip)
                .layout(Layout::right_to_left(Align::Center)),
        );
        right.spacing_mut().item_spacing.x = space::XS;
        let meeting = if in_meeting {
            let look = HeaderLook::Tinted {
                fill: p.danger_soft,
                color: p.danger,
            };
            header_button(
                &mut right,
                Icon::People,
                "End meeting",
                "End meeting",
                look,
                compact,
                f32::INFINITY,
            )
        } else {
            header_button(
                &mut right,
                Icon::People,
                "Meeting",
                "Meeting",
                HeaderLook::Ghost,
                compact,
                f32::INFINITY,
            )
        };
        if meeting.clicked() {
            action = Some(Action::ToggleMeeting);
        }
        if running.is_some()
            && !on_break
            && !in_meeting
            && header_button(
                &mut right,
                Icon::Cup,
                "Take break",
                "Take break",
                HeaderLook::Ghost,
                compact,
                f32::INFINITY,
            )
            .clicked()
        {
            action = Some(Action::TakeBreak);
        }
        if let Some((_, label)) = &self.resume
            && (running.is_none() || on_break)
        {
            let name = format!("Resume {label}");
            let look = HeaderLook::Tinted {
                fill: p.accent_soft,
                color: p.accent,
            };
            if header_button(
                &mut right,
                Icon::Play,
                &name,
                &name,
                look,
                compact,
                size::RESUME_LABEL,
            )
            .clicked()
            {
                action = Some(Action::Resume);
            }
        }
        right.add_space(space::XL - space::XS);
        let office = office_time(&sessions, today, &tz, now);
        let (parts, share) = office_parts(office, settings, today.weekday(), compact);
        let tone = office_tone(office, settings, today.weekday());
        office_meter(
            &mut right,
            [&parts[0], &parts[1], &parts[2]],
            share,
            tone_color(tone, &p),
            &office_text(office, settings, today.weekday()),
        );
        let taken = right.min_rect().width();

        let left_rect = Rect::from_min_max(
            strip.min,
            pos2(strip.right() - taken - space::XL, strip.bottom()),
        );
        let mut left = ui.new_child(
            UiBuilder::new()
                .max_rect(left_rect)
                .layout(Layout::left_to_right(Align::Center)),
        );
        left.spacing_mut().item_spacing.x = space::SM;
        let block = active_block(blocks, now, &tz).map(|b| {
            format!(
                "{} block until {}",
                block_name(store, b),
                time_of_day(b.end_min)
            )
        });
        match &running {
            Some(session) => {
                let (title, parent) = session_title(store, session);
                let next = in_meeting
                    .then(|| {
                        self.resume
                            .as_ref()
                            .map(|(_, label)| format!("Then back to {label}"))
                    })
                    .flatten();
                let subtitle = [parent, next, block]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" · ");
                let time = clock_text(tracked_time(
                    &sessions,
                    TimerTarget::resuming(session),
                    today,
                    &tz,
                    now,
                ));
                let room = left_rect.width()
                    - hero_time_width(&left, &time)
                    - size::STOP_BUTTON
                    - space::MD
                    - space::SM;
                running_title(
                    &mut left,
                    Some(timer_color(store, session, p.accent)),
                    &title,
                    (!subtitle.is_empty()).then_some(subtitle.as_str()),
                    room,
                );
                left.add_space(space::MD - space::SM);
                hero_time(&mut left, &time, p.text);
                if stop_button(&mut left).clicked() {
                    action = Some(Action::Stop);
                }
            }
            None => {
                let hint = block.unwrap_or_else(|| IDLE_HINT.to_owned());
                running_title(
                    &mut left,
                    None,
                    "Nothing running",
                    Some(&hint),
                    left_rect.width(),
                );
            }
        }

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

    /// Starts or ends the meeting for a Teams call and says so in a toast.
    /// Does nothing when the timer is already in the state asked for.
    pub fn call_meeting(
        &mut self,
        action: CallAction,
        store: &dyn SessionRepo,
        notifier: &dyn Notifier,
    ) {
        let before = match store.running() {
            Ok(running) => running,
            Err(e) => {
                self.message = Some(e.to_string());
                return;
            }
        };
        let in_meeting = before.as_ref().is_some_and(Session::is_meeting);
        if in_meeting != (action == CallAction::EndMeeting) {
            return;
        }
        self.apply(Action::ToggleMeeting, store);
        if self.message.is_some() {
            return;
        }
        let body = match action {
            // A break isn't resumed after a meeting, so it isn't named.
            CallAction::StartMeeting => match before.filter(|s| !s.is_break()) {
                Some(work) => format!(
                    "Tracking it as a meeting. {} resumes when the call ends.",
                    session_label(store, &work)
                ),
                None => "Tracking it as a meeting.".to_owned(),
            },
            CallAction::EndMeeting => match store.running().ok().flatten() {
                Some(resumed) => format!("Back to {}.", session_label(store, &resumed)),
                None => "Meeting time saved.".to_owned(),
            },
        };
        let title = match action {
            CallAction::StartMeeting => "Teams meeting detected",
            CallAction::EndMeeting => "Teams meeting ended",
        };
        notifier.notify(title, &body);
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

/// The title and, for a sub-task, its parent's title.
fn session_title(store: &dyn SessionRepo, session: &Session) -> (String, Option<String>) {
    let parent = session
        .task_id
        .and_then(|id| store.get(id).ok())
        .and_then(|task| task.parent_id)
        .and_then(|id| store.get(id).ok())
        .map(|parent| parent.title);
    (session_label(store, session), parent)
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

/// Where today's office time stands, for the header meter's colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OfficeTone {
    /// Short of the target (or nothing yet on a day off).
    Under,
    /// The target is met, with room before the soft ceiling.
    Met,
    /// Within half an hour of the soft ceiling.
    NearCeiling,
    /// At or past the soft ceiling.
    Over,
    /// Office time on a day off: all of it is overtime.
    DayOff,
}

pub fn office_tone(
    office: SignedDuration,
    settings: &Settings,
    weekday: jiff::civil::Weekday,
) -> OfficeTone {
    if !settings.workdays.contains(&weekday) {
        return if office.is_positive() {
            OfficeTone::DayOff
        } else {
            OfficeTone::Under
        };
    }
    let target = SignedDuration::from_mins(i64::from(settings.office_target_min));
    let ceiling = SignedDuration::from_mins(i64::from(settings.office_ceiling_min));
    if office < target {
        OfficeTone::Under
    } else if office >= ceiling {
        OfficeTone::Over
    } else if office >= ceiling - CEILING_NEAR {
        OfficeTone::NearCeiling
    } else {
        OfficeTone::Met
    }
}

fn tone_color(tone: OfficeTone, p: &Palette) -> Color32 {
    match tone {
        OfficeTone::Under => p.accent,
        OfficeTone::Met => p.success,
        OfficeTone::NearCeiling | OfficeTone::DayOff => p.warning,
        OfficeTone::Over => p.danger,
    }
}

/// The meter's line in three parts (`Office `, `6h 05m`, ` of 8h`) and how
/// full its bar is. Compact drops the lead.
fn office_parts(
    office: SignedDuration,
    settings: &Settings,
    weekday: jiff::civil::Weekday,
    compact: bool,
) -> ([String; 3], f32) {
    let lead = if compact { "" } else { "Office " }.to_owned();
    let worked = hours_text(office);
    if !settings.workdays.contains(&weekday) {
        let share = if office.is_positive() { 1.0 } else { 0.0 };
        return ([lead, worked, " · day off".to_owned()], share);
    }
    let rest = match office_status(office, settings, weekday) {
        OfficeStatus::Overtime { minutes } => format!(" · {minutes}m overtime"),
        _ => format!(" of {}", target_text(settings.office_target_min)),
    };
    let share = office.as_mins() as f32 / settings.office_target_min.max(1) as f32;
    ([lead, worked, rest], share)
}

/// The running session's category colour, falling back to `default`.
fn timer_color(store: &dyn SessionRepo, session: &Session, default: Color32) -> Color32 {
    store
        .list()
        .ok()
        .and_then(|cats| cats.into_iter().find(|c| c.meta.id == session.category_id))
        .map_or(default, |c| color32(c.color))
}
