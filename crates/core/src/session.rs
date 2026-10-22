use jiff::civil::{Date, Weekday};
use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp};

use crate::{CategoryKind, Id, Meta, Settings};

/// One stretch of tracked time. `ended_at` is `None` while it runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Session {
    pub meta: Meta,
    /// `None` for meetings, breaks and quick-started category timers.
    pub task_id: Option<Id>,
    pub category_id: Id,
    /// The category's kind when the session started.
    pub kind: CategoryKind,
    pub started_at: Timestamp,
    pub ended_at: Option<Timestamp>,
}

impl Session {
    pub fn is_running(&self) -> bool {
        self.ended_at.is_none()
    }

    pub fn is_break(&self) -> bool {
        self.kind == CategoryKind::Break
    }

    pub fn is_meeting(&self) -> bool {
        self.kind == CategoryKind::Meeting
    }

    pub fn counts_as_office(&self) -> bool {
        matches!(self.kind, CategoryKind::Office | CategoryKind::Meeting)
    }

    /// End time, treating a running session as ending `now`.
    pub fn end_or(&self, now: Timestamp) -> Timestamp {
        self.ended_at.unwrap_or(now)
    }

    /// How much of this session falls on the local calendar `day`.
    pub fn time_within(&self, day: Date, tz: &TimeZone, now: Timestamp) -> SignedDuration {
        let Some((day_start, day_end)) = day_bounds(day, tz) else {
            return SignedDuration::ZERO;
        };
        let start = self.started_at.max(day_start);
        let end = self.end_or(now).min(day_end);
        if end > start {
            end.duration_since(start)
        } else {
            SignedDuration::ZERO
        }
    }
}

/// Start of `day` and of the next day, as instants in `tz`.
fn day_bounds(day: Date, tz: &TimeZone) -> Option<(Timestamp, Timestamp)> {
    let start = day.to_zoned(tz.clone()).ok()?.timestamp();
    let end = day.tomorrow().ok()?.to_zoned(tz.clone()).ok()?.timestamp();
    Some((start, end))
}

/// Office time on a local day: office and meeting sessions, breaks excluded.
pub fn office_time(
    sessions: &[Session],
    day: Date,
    tz: &TimeZone,
    now: Timestamp,
) -> SignedDuration {
    sessions
        .iter()
        .filter(|s| s.counts_as_office())
        .map(|s| s.time_within(day, tz, now))
        .fold(SignedDuration::ZERO, |sum, d| sum.saturating_add(d))
}

/// What a timer runs on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimerTarget {
    Task(Id),
    /// Quick start: a category with no task (e.g. Family time).
    Category(Id),
}

impl TimerTarget {
    /// What to resume after a meeting or break interrupted `session`.
    pub fn resuming(session: &Session) -> Self {
        session
            .task_id
            .map_or(Self::Category(session.category_id), Self::Task)
    }
}

/// The result of starting a session: the one it ended (if any) and the new one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Switch {
    pub ended: Option<Session>,
    pub started: Session,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MeetingToggle {
    /// A meeting started; `paused` is the session it interrupted.
    Started {
        paused: Option<Session>,
    },
    Ended {
        meeting: Session,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OfficeStatus {
    UnderTarget,
    TargetReached,
    SoftCeiling,
    Overtime { minutes: i64 },
}

/// Where today's office time stands. Workdays have an 8h target and a 9h
/// soft ceiling (by default); on other days every office minute is overtime.
pub fn office_status(
    office: SignedDuration,
    settings: &Settings,
    weekday: Weekday,
) -> OfficeStatus {
    let minutes = office.as_mins();
    if !settings.workdays.contains(&weekday) {
        return if minutes > 0 {
            OfficeStatus::Overtime { minutes }
        } else {
            OfficeStatus::UnderTarget
        };
    }
    let (target, ceiling) = (
        i64::from(settings.office_target_min),
        i64::from(settings.office_ceiling_min),
    );
    match minutes {
        m if m < target => OfficeStatus::UnderTarget,
        m if m < ceiling => OfficeStatus::TargetReached,
        m if m == ceiling => OfficeStatus::SoftCeiling,
        m => OfficeStatus::Overtime {
            minutes: m - ceiling,
        },
    }
}
