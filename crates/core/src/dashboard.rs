//! Dashboard figures: today's timeline, office time per day of the week,
//! time per category and distractions over a range. Pure functions over
//! sessions and events; no aggregate of its own.

use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp, ToSpan};

use crate::{
    CategoryKind, DistractionEvent, DistractionRule, Id, OfficeStatus, Session, Settings,
    office_status, office_time,
};

const DAY_MIN: u16 = 24 * 60;
/// The timeline shows at least this many minutes.
const MIN_WINDOW: u16 = 8 * 60;
/// The window of a day with nothing tracked: 09:00-17:00.
const EMPTY_WINDOW: (u16, u16) = (9 * 60, 17 * 60);

/// One session's part of a local day, for the timeline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimelineSegment {
    pub task_id: Option<Id>,
    pub category_id: Id,
    pub kind: CategoryKind,
    /// Still running (it ends at "now").
    pub running: bool,
    /// Local clock minutes after midnight, minutes floored; `1440` is midnight
    /// at the end of the day.
    pub from_min: u16,
    pub to_min: u16,
}

/// The sessions with time on the local `day`, clipped to it and in start
/// order, a running session up to `now`.
pub fn day_timeline(
    sessions: &[Session],
    day: Date,
    tz: &TimeZone,
    now: Timestamp,
) -> Vec<TimelineSegment> {
    let Some((day_start, day_end)) = local_range(day, day.tomorrow().ok(), tz) else {
        return Vec::new();
    };
    let minute = |at: Timestamp| {
        if at >= day_end {
            DAY_MIN
        } else {
            let local = at.to_zoned(tz.clone());
            (local.hour() as u16) * 60 + local.minute() as u16
        }
    };
    let mut segments: Vec<(Timestamp, TimelineSegment)> = sessions
        .iter()
        .filter(|s| s.time_within(day, tz, now).is_positive())
        .map(|s| {
            let (from, to) = (s.started_at.max(day_start), s.end_or(now).min(day_end));
            let segment = TimelineSegment {
                task_id: s.task_id,
                category_id: s.category_id,
                kind: s.kind,
                running: s.is_running(),
                from_min: minute(from),
                to_min: minute(to),
            };
            (from, segment)
        })
        .collect();
    segments.sort_by_key(|(from, _)| *from);
    segments.into_iter().map(|(_, s)| s).collect()
}

/// The local minutes `[from, to)` the timeline shows: from the hour the first
/// segment starts to the hour after the last one ends, at least eight hours
/// (grown later, then earlier), within the day.
pub fn timeline_window(segments: &[TimelineSegment]) -> (u16, u16) {
    let (Some(first), Some(last)) = (
        segments.iter().map(|s| s.from_min).min(),
        segments.iter().map(|s| s.to_min).max(),
    ) else {
        return EMPTY_WINDOW;
    };
    let start = first / 60 * 60;
    let mut end = last.div_ceil(60) * 60;
    if end - start >= MIN_WINDOW {
        return (start, end.min(DAY_MIN));
    }
    end = (start + MIN_WINDOW).min(DAY_MIN);
    (end - MIN_WINDOW, end)
}

/// Office time on one day of a week.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DayOffice {
    pub date: Date,
    pub office: SignedDuration,
    /// The daily office target; `None` on a day off.
    pub target: Option<SignedDuration>,
    /// Time past the soft ceiling, or all office time on a day off.
    pub overtime: SignedDuration,
}

/// Office time for each of the seven days from `first_day`.
pub fn office_by_day(
    sessions: &[Session],
    first_day: Date,
    tz: &TimeZone,
    now: Timestamp,
    settings: &Settings,
) -> Vec<DayOffice> {
    (0..7)
        .filter_map(|i| first_day.checked_add(i.days()).ok())
        .map(|date| {
            let office = office_time(sessions, date, tz, now);
            let workday = settings.workdays.contains(&date.weekday());
            let overtime = match office_status(office, settings, date.weekday()) {
                OfficeStatus::Overtime { minutes } => SignedDuration::from_mins(minutes),
                _ => SignedDuration::ZERO,
            };
            DayOffice {
                date,
                office,
                target: workday
                    .then(|| SignedDuration::from_mins(i64::from(settings.office_target_min))),
                overtime,
            }
        })
        .collect()
}

/// Time in one category over a range.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CategoryTime {
    pub category_id: Id,
    pub time: SignedDuration,
}

/// Time per category over the local dates `[from, to)`, breaks left out,
/// most time first (ties in the order the categories were first tracked).
pub fn category_totals(
    sessions: &[Session],
    from: Date,
    to: Date,
    tz: &TimeZone,
    now: Timestamp,
) -> Vec<CategoryTime> {
    let Some((start, end)) = local_range(from, Some(to), tz) else {
        return Vec::new();
    };
    let mut totals: Vec<CategoryTime> = Vec::new();
    for s in sessions.iter().filter(|s| !s.is_break()) {
        let time = overlap(s.started_at, s.end_or(now), start, end);
        if !time.is_positive() {
            continue;
        }
        match totals.iter_mut().find(|t| t.category_id == s.category_id) {
            Some(total) => total.time = total.time.saturating_add(time),
            None => totals.push(CategoryTime {
                category_id: s.category_id,
                time,
            }),
        }
    }
    totals.sort_by_key(|t| std::cmp::Reverse(t.time));
    totals
}

/// Time on the local `day` in sessions whose kind passes `keep`.
pub fn kind_time(
    sessions: &[Session],
    day: Date,
    tz: &TimeZone,
    now: Timestamp,
    keep: impl Fn(CategoryKind) -> bool,
) -> SignedDuration {
    sessions
        .iter()
        .filter(|s| keep(s.kind))
        .map(|s| s.time_within(day, tz, now))
        .fold(SignedDuration::ZERO, SignedDuration::saturating_add)
}

/// Time one distraction rule took.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleTime {
    /// The rule's pattern, or `(deleted rule)`.
    pub name: String,
    pub time: SignedDuration,
}

/// Distracted time over a range: the total and the time per rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DistractionTotals {
    pub total: SignedDuration,
    /// Rules with at least a minute, in rule order; deleted rules last.
    pub rules: Vec<RuleTime>,
}

/// Distractions over the local dates `[from, to)`, an open event up to `now`.
pub fn distraction_totals(
    events: &[DistractionEvent],
    rules: &[DistractionRule],
    from: Date,
    to: Date,
    tz: &TimeZone,
    now: Timestamp,
) -> DistractionTotals {
    let Some((start, end)) = local_range(from, Some(to), tz) else {
        return DistractionTotals {
            total: SignedDuration::ZERO,
            rules: Vec::new(),
        };
    };
    let times: Vec<(Id, SignedDuration)> = events
        .iter()
        .map(|e| {
            let time = overlap(e.started_at, e.ended_at.unwrap_or(now), start, end);
            (e.rule_id, time)
        })
        .filter(|(_, t)| t.is_positive())
        .collect();
    let sum_where = |keep: &dyn Fn(Id) -> bool| {
        times
            .iter()
            .filter(|(id, _)| keep(*id))
            .map(|(_, t)| *t)
            .fold(SignedDuration::ZERO, SignedDuration::saturating_add)
    };
    let mut lines: Vec<RuleTime> = rules
        .iter()
        .map(|r| RuleTime {
            name: r.pattern.clone(),
            time: sum_where(&|id| id == r.meta.id),
        })
        .collect();
    lines.push(RuleTime {
        name: "(deleted rule)".to_owned(),
        time: sum_where(&|id| !rules.iter().any(|r| r.meta.id == id)),
    });
    lines.retain(|l| l.time.as_mins() > 0);
    DistractionTotals {
        total: sum_where(&|_| true),
        rules: lines,
    }
}

/// The instants of local midnight at `from` and at `to` (default: `from`'s end).
fn local_range(from: Date, to: Option<Date>, tz: &TimeZone) -> Option<(Timestamp, Timestamp)> {
    let instant = |d: Date| d.to_zoned(tz.clone()).ok().map(|z| z.timestamp());
    Some((instant(from)?, instant(to?)?))
}

/// How much of `[start, end)` falls within `[from, to)`.
fn overlap(start: Timestamp, end: Timestamp, from: Timestamp, to: Timestamp) -> SignedDuration {
    let (a, b) = (start.max(from), end.min(to));
    if b > a {
        b.duration_since(a)
    } else {
        SignedDuration::ZERO
    }
}
