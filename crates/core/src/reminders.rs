use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp};

use std::collections::HashSet;

use crate::{Block, Id, OfficeStatus, Session, Settings, office_status, office_time};

/// Something the user should be told about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notice {
    /// Office time reached the daily target (8h by default).
    TargetReached,
    /// Office time reached the soft ceiling (9h by default).
    SoftCeiling,
    /// Continuous work reached the break interval.
    BreakDue,
    /// The running break has lasted the break length.
    BreakOver,
    /// A scheduled block began and its timer isn't running.
    BlockStarted { block: Id },
}

/// Everything one reminder check looks at.
pub struct Tick<'a> {
    /// Recent sessions (at least today's, plus yesterday's for a stretch
    /// that crosses midnight).
    pub sessions: &'a [Session],
    pub now: Timestamp,
    pub tz: &'a TimeZone,
    pub settings: &'a Settings,
    /// The running task's own break interval, if it has one.
    pub break_interval_min: Option<u32>,
    /// Scheduled blocks, for start reminders.
    pub blocks: &'a [Block],
}

/// How often an ignored break reminder repeats.
const BREAK_REPEAT: SignedDuration = SignedDuration::from_mins(5);

/// Decides when to notify. Pure: it only remembers what it already said, so
/// each notice fires once per crossing and nothing re-fires on app start.
#[derive(Debug, Default)]
pub struct Reminders {
    /// Office progress (0 under target, 1 past target, 2 past ceiling) seen
    /// on the last tick, and the day it was for.
    office: Option<(Date, u8)>,
    /// Start of the stretch the last break reminder was for, and when it fired.
    break_due: Option<(Timestamp, Timestamp)>,
    /// The break session `BreakOver` already fired for.
    break_over_for: Option<Id>,
    /// Blocks already handled today (reminded, or skipped as already running),
    /// keyed with their times so a block moved to later reminds again.
    blocks_handled: HashSet<(Id, Date, u16, u16)>,
}

impl Reminders {
    pub fn tick(&mut self, t: Tick<'_>) -> Vec<Notice> {
        let mut notices = Vec::new();
        self.check_office(&t, &mut notices);
        self.check_breaks(&t, &mut notices);
        self.check_blocks(&t, &mut notices);
        notices
    }

    /// Reminds once per block per day while it's on, unless its category's
    /// timer is already running.
    fn check_blocks(&mut self, t: &Tick<'_>, notices: &mut Vec<Notice>) {
        let local = t.now.to_zoned(t.tz.clone());
        let today = local.date();
        self.blocks_handled.retain(|(_, day, _, _)| *day == today);
        let running = t.sessions.iter().find(|s| s.is_running());
        for block in t.blocks.iter().filter(|b| b.is_on(&local)) {
            let first_time =
                self.blocks_handled
                    .insert((block.meta.id, today, block.start_min, block.end_min));
            let already_running = running.is_some_and(|s| s.category_id == block.category_id);
            if first_time && !already_running {
                notices.push(Notice::BlockStarted {
                    block: block.meta.id,
                });
            }
        }
    }

    fn check_office(&mut self, t: &Tick<'_>, notices: &mut Vec<Notice>) {
        let today = t.now.to_zoned(t.tz.clone()).date();
        let weekday = today.weekday();
        let progress = if t.settings.workdays.contains(&weekday) {
            match office_status(
                office_time(t.sessions, today, t.tz, t.now),
                t.settings,
                weekday,
            ) {
                OfficeStatus::UnderTarget => 0,
                OfficeStatus::TargetReached => 1,
                OfficeStatus::SoftCeiling | OfficeStatus::Overtime { .. } => 2,
            }
        } else {
            0
        };
        if let Some((day, before)) = self.office
            && day == today
        {
            if before < 1 && progress >= 1 {
                notices.push(Notice::TargetReached);
            }
            if before < 2 && progress >= 2 {
                notices.push(Notice::SoftCeiling);
            }
        }
        self.office = Some((today, progress));
    }

    fn check_breaks(&mut self, t: &Tick<'_>, notices: &mut Vec<Notice>) {
        let break_length = minutes(t.settings.break_length_min);
        let Some(running) = t.sessions.iter().find(|s| s.is_running()) else {
            self.break_due = None;
            return;
        };

        if running.is_break() {
            self.break_due = None;
            if t.now.duration_since(running.started_at) >= break_length
                && self.break_over_for != Some(running.meta.id)
            {
                notices.push(Notice::BreakOver);
                self.break_over_for = Some(running.meta.id);
            }
            return;
        }

        let stretch_start = stretch_start(t.sessions, running, break_length);
        let interval = minutes(
            t.break_interval_min
                .unwrap_or(t.settings.break_interval_min),
        );
        if t.now.duration_since(stretch_start) < interval {
            return;
        }
        let due = match self.break_due {
            Some((start, last)) if start == stretch_start => {
                t.now.duration_since(last) >= BREAK_REPEAT
            }
            _ => true,
        };
        if due {
            notices.push(Notice::BreakDue);
            self.break_due = Some((stretch_start, t.now));
        }
    }
}

/// Start of the continuous work that `running` belongs to: earlier non-break
/// sessions count while each gap is shorter than a break.
fn stretch_start(
    sessions: &[Session],
    running: &Session,
    break_length: SignedDuration,
) -> Timestamp {
    let mut earlier: Vec<(&Session, Timestamp)> = sessions
        .iter()
        .filter_map(|s| s.ended_at.map(|end| (s, end)))
        .filter(|(_, end)| *end <= running.started_at)
        .collect();
    earlier.sort_by_key(|(_, end)| std::cmp::Reverse(*end));

    let mut start = running.started_at;
    for (session, end) in earlier {
        if session.is_break() || start.duration_since(end) >= break_length {
            break;
        }
        start = session.started_at;
    }
    start
}

fn minutes(m: u32) -> SignedDuration {
    SignedDuration::from_mins(i64::from(m))
}
