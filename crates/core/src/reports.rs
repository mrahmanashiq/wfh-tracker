//! Plain-text day reports: the office report for Teams and the private
//! "My time" report. Pure functions over one day's data.

use std::collections::HashMap;

use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp};

use crate::{
    Category, CategoryKind, Id, OfficeStatus, Session, Settings, Task, office_status, office_time,
};

/// Everything a report needs about one local day.
pub struct ReportInput<'a> {
    pub day: Date,
    pub tz: &'a TimeZone,
    /// Running sessions count up to this instant.
    pub now: Timestamp,
    /// Sessions around the day (others contribute nothing).
    pub sessions: &'a [Session],
    /// Live tasks; sessions of tasks not listed are reported as deleted.
    pub tasks: &'a [Task],
    pub categories: &'a [Category],
    pub settings: &'a Settings,
}

/// `50m`, or `2h 05m` from an hour on.
pub fn duration_text(d: SignedDuration) -> String {
    let m = d.as_mins().max(0);
    match m / 60 {
        0 => format!("{m}m"),
        h => format!("{h}h {:02}m", m % 60),
    }
}

/// `7h 45m`: always hours and minutes, for totals.
fn hours_text(d: SignedDuration) -> String {
    let m = d.as_mins().max(0);
    format!("{}h {:02}m", m / 60, m % 60)
}

fn date_text(day: Date) -> String {
    day.strftime("%a %-d %b %Y").to_string()
}

/// The office report: what was worked on or finished, meetings, overtime.
pub fn office_report(input: &ReportInput<'_>) -> String {
    let day = Day::new(input);
    let office = office_time(input.sessions, input.day, input.tz, input.now);
    let weekday = input.day.weekday();
    let off_day = !input.settings.workdays.contains(&weekday);
    let target = if off_day {
        "(day off)".to_owned()
    } else {
        format!("(target {})", target_text(input.settings.office_target_min))
    };

    let mut lines = vec![
        format!(
            "Office report — {} — {} {target}",
            date_text(input.day),
            hours_text(office)
        ),
        String::new(),
    ];
    lines.extend(day.task_lines(input, |kind| kind == CategoryKind::Office));

    let deleted = day.deleted_time(CategoryKind::Office);
    if deleted.is_positive() {
        lines.push(format!("(deleted task) — {}", duration_text(deleted)));
    }
    let untasked = day.untasked_time(|s| s.kind == CategoryKind::Office);
    if untasked.is_positive() {
        lines.push(format!("Other office work — {}", duration_text(untasked)));
    }

    let meetings: Vec<SignedDuration> = day
        .sessions
        .iter()
        .filter(|(s, _)| s.is_meeting())
        .map(|(_, t)| *t)
        .collect();
    lines.push(match meetings.len() {
        0 => "Meetings — none".to_owned(),
        n => format!("Meetings — {n} ({})", duration_text(sum(meetings))),
    });

    lines.push(match office_status(office, input.settings, weekday) {
        OfficeStatus::Overtime { minutes } if off_day => {
            format!(
                "Overtime — {}",
                hours_text(SignedDuration::from_mins(minutes))
            )
        }
        OfficeStatus::Overtime { minutes } => {
            format!(
                "Overtime — {}",
                duration_text(SignedDuration::from_mins(minutes))
            )
        }
        _ => "Overtime — none".to_owned(),
    });
    lines.join("\n")
}

/// The private report: learning by task, then personal time by category.
pub fn my_time_report(input: &ReportInput<'_>) -> String {
    let day = Day::new(input);
    let mut sections = Vec::new();

    let mut learning = day.task_lines(input, |kind| kind == CategoryKind::Learning);
    for category in input
        .categories
        .iter()
        .filter(|c| c.kind == CategoryKind::Learning)
    {
        let time = day.untasked_time(|s| s.category_id == category.meta.id);
        if time.is_positive() {
            learning.push(format!(
                "{} (no task) — {}",
                category.name,
                duration_text(time)
            ));
        }
    }
    let deleted = day.deleted_time(CategoryKind::Learning);
    if deleted.is_positive() {
        learning.push(format!("(deleted task) — {}", duration_text(deleted)));
    }
    let learning_total = day.kind_time(|k| k == CategoryKind::Learning);
    if learning_total.is_positive() {
        sections.push(section("Learning", learning_total, learning));
    }

    let mut personal: Vec<String> = input
        .categories
        .iter()
        .filter(|c| is_personal(c.kind))
        .filter_map(|c| {
            let time = day.time_where(|s| s.category_id == c.meta.id);
            time.is_positive()
                .then(|| format!("{} — {}", c.name, duration_text(time)))
        })
        .collect();
    let personal_total = day.kind_time(is_personal);
    let listed = sum(input
        .categories
        .iter()
        .filter(|c| is_personal(c.kind))
        .map(|c| day.time_where(|s| s.category_id == c.meta.id)));
    let unlisted = personal_total
        .checked_sub(listed)
        .unwrap_or(SignedDuration::ZERO);
    if unlisted.is_positive() {
        personal.push(format!("(deleted category) — {}", duration_text(unlisted)));
    }
    if personal_total.is_positive() {
        sections.push(section("Personal", personal_total, personal));
    }

    let body = if sections.is_empty() {
        "Nothing tracked for yourself this day.".to_owned()
    } else {
        sections.join("\n\n")
    };
    format!("My time — {}\n\n{body}", date_text(input.day))
}

fn is_personal(kind: CategoryKind) -> bool {
    matches!(kind, CategoryKind::Personal | CategoryKind::Custom)
}

fn section(title: &str, total: SignedDuration, lines: Vec<String>) -> String {
    let mut out = vec![format!("{title} — {}", hours_text(total))];
    out.extend(lines.into_iter().map(|l| format!("  {l}")));
    out.join("\n")
}

fn target_text(minutes: u32) -> String {
    match minutes % 60 {
        0 => format!("{}h", minutes / 60),
        m => format!("{}h {m:02}m", minutes / 60),
    }
}

fn sum(times: impl IntoIterator<Item = SignedDuration>) -> SignedDuration {
    times
        .into_iter()
        .fold(SignedDuration::ZERO, SignedDuration::saturating_add)
}

/// One day's sessions with their in-day durations, plus task lookups.
struct Day<'a> {
    sessions: Vec<(&'a Session, SignedDuration)>,
    tasks: HashMap<Id, &'a Task>,
    kinds: HashMap<Id, CategoryKind>,
    day: Date,
    tz: &'a TimeZone,
}

impl<'a> Day<'a> {
    fn new(input: &ReportInput<'a>) -> Self {
        Self {
            sessions: input
                .sessions
                .iter()
                .map(|s| (s, s.time_within(input.day, input.tz, input.now)))
                .filter(|(_, t)| t.is_positive())
                .collect(),
            tasks: input.tasks.iter().map(|t| (t.meta.id, t)).collect(),
            kinds: input
                .categories
                .iter()
                .map(|c| (c.meta.id, c.kind))
                .collect(),
            day: input.day,
            tz: input.tz,
        }
    }

    fn time_where(&self, keep: impl Fn(&Session) -> bool) -> SignedDuration {
        sum(self
            .sessions
            .iter()
            .filter(|(s, _)| keep(s))
            .map(|(_, t)| *t))
    }

    fn kind_time(&self, keep: impl Fn(CategoryKind) -> bool) -> SignedDuration {
        self.time_where(|s| keep(s.kind))
    }

    /// Time on a task that day, in sessions of the matching kinds only, so a
    /// task's lines always add up to the section totals (which use session kinds).
    fn task_time(&self, id: Id, kind: &dyn Fn(CategoryKind) -> bool) -> SignedDuration {
        self.time_where(|s| s.task_id == Some(id) && kind(s.kind))
    }

    fn untasked_time(&self, keep: impl Fn(&Session) -> bool) -> SignedDuration {
        self.time_where(|s| s.task_id.is_none() && keep(s))
    }

    fn deleted_time(&self, kind: CategoryKind) -> SignedDuration {
        self.time_where(|s| {
            s.kind == kind && s.task_id.is_some_and(|id| !self.tasks.contains_key(&id))
        })
    }

    fn local_date(&self, at: Timestamp) -> Date {
        at.to_zoned(self.tz.clone()).date()
    }

    /// Done by the end of the report day (a task ticked later was still open then).
    fn done_by_day(&self, task: &Task) -> bool {
        task.done_at
            .is_some_and(|at| self.local_date(at) <= self.day)
    }

    /// Worked on in sessions of these kinds that day, or ticked that day while
    /// in a category of these kinds.
    fn relevant(&self, task: &Task, kind: &dyn Fn(CategoryKind) -> bool) -> bool {
        self.task_time(task.meta.id, kind).is_positive()
            || (task
                .done_at
                .is_some_and(|at| self.local_date(at) == self.day)
                && self.kinds.get(&task.category_id).is_some_and(|k| kind(*k)))
    }

    /// Lines for tasks with time of the matching kinds: a parent with its total
    /// and its relevant sub-tasks indented, or a single line.
    fn task_lines(
        &self,
        input: &ReportInput<'_>,
        kind: impl Fn(CategoryKind) -> bool,
    ) -> Vec<String> {
        let kind: &dyn Fn(CategoryKind) -> bool = &kind;
        let mut tops: Vec<&Task> = input
            .tasks
            .iter()
            .filter(|t| t.parent_id.is_none())
            .collect();
        tops.sort_by_key(|t| t.meta.id);

        let mut lines = Vec::new();
        for top in tops {
            let mut children: Vec<&Task> = input
                .tasks
                .iter()
                .filter(|t| t.parent_id == Some(top.meta.id))
                .collect();
            children.sort_by_key(|t| t.meta.id);
            let shown: Vec<&Task> = children
                .iter()
                .copied()
                .filter(|t| self.relevant(t, kind))
                .collect();

            if shown.is_empty() {
                if self.relevant(top, kind) {
                    lines.push(self.task_line(top, kind));
                }
                continue;
            }
            let total = sum(children.iter().map(|t| self.task_time(t.meta.id, kind)))
                .saturating_add(self.task_time(top.meta.id, kind));
            lines.push(with_time(&top.title, total));
            lines.extend(
                shown
                    .into_iter()
                    .map(|t| format!("  {}", self.task_line(t, kind))),
            );
        }
        lines
    }

    /// `✅ Title — 50m`: ✅ when done by the end of the report day.
    fn task_line(&self, task: &Task, kind: &dyn Fn(CategoryKind) -> bool) -> String {
        let mark = if self.done_by_day(task) { "✅" } else { "⏳" };
        with_time(
            &format!("{mark} {}", task.title),
            self.task_time(task.meta.id, kind),
        )
    }
}

/// `text — 50m`, or just `text` when no time was tracked (e.g. ticked only).
fn with_time(text: &str, time: SignedDuration) -> String {
    if time.is_positive() {
        format!("{text} — {}", duration_text(time))
    } else {
        text.to_owned()
    }
}
