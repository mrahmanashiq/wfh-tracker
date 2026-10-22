#![allow(clippy::unwrap_used)] // test code may unwrap (SPEC: Code Style)

use jiff::civil::date;
use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp};
use wt_core::{CategoryKind, Id, Meta, Session, TimerTarget, tracked_time};

fn ts(s: &str) -> Timestamp {
    s.parse().unwrap()
}

fn session(
    task: Option<Id>,
    category: Id,
    kind: CategoryKind,
    from: &str,
    to: Option<&str>,
) -> Session {
    let start = ts(from);
    Session {
        meta: Meta {
            id: Id::new(),
            created_at: start,
            updated_at: start,
            deleted_at: None,
        },
        task_id: task,
        category_id: category,
        kind,
        started_at: start,
        ended_at: to.map(ts),
    }
}

#[test]
fn a_tasks_time_adds_up_every_stretch_today_including_the_running_one() {
    let (task, other, office, meeting) = (Id::new(), Id::new(), Id::new(), Id::new());
    let sessions = [
        session(
            Some(task),
            office,
            CategoryKind::Office,
            "2026-10-22T03:00:00Z",
            Some("2026-10-22T03:30:00Z"),
        ),
        session(
            None,
            meeting,
            CategoryKind::Meeting,
            "2026-10-22T03:30:00Z",
            Some("2026-10-22T03:40:00Z"),
        ),
        session(
            Some(other),
            office,
            CategoryKind::Office,
            "2026-10-22T03:40:00Z",
            Some("2026-10-22T03:50:00Z"),
        ),
        session(
            Some(task),
            office,
            CategoryKind::Office,
            "2026-10-22T03:50:00Z",
            None,
        ),
    ];

    let total = tracked_time(
        &sessions,
        TimerTarget::Task(task),
        date(2026, 10, 22),
        &TimeZone::UTC,
        ts("2026-10-22T03:55:00Z"),
    );

    assert_eq!(total, SignedDuration::from_mins(35));
}

#[test]
fn a_quick_started_category_counts_only_its_task_less_sessions() {
    let (family, task) = (Id::new(), Id::new());
    let sessions = [
        session(
            None,
            family,
            CategoryKind::Personal,
            "2026-10-22T12:00:00Z",
            Some("2026-10-22T12:20:00Z"),
        ),
        session(
            Some(task),
            family,
            CategoryKind::Personal,
            "2026-10-22T12:20:00Z",
            Some("2026-10-22T13:00:00Z"),
        ),
    ];

    let total = tracked_time(
        &sessions,
        TimerTarget::Category(family),
        date(2026, 10, 22),
        &TimeZone::UTC,
        ts("2026-10-22T14:00:00Z"),
    );

    assert_eq!(total, SignedDuration::from_mins(20));
}

#[test]
fn yesterdays_time_is_not_counted_today() {
    let task = Id::new();
    let sessions = [session(
        Some(task),
        Id::new(),
        CategoryKind::Office,
        "2026-10-21T10:00:00Z",
        Some("2026-10-21T11:00:00Z"),
    )];

    let total = tracked_time(
        &sessions,
        TimerTarget::Task(task),
        date(2026, 10, 22),
        &TimeZone::UTC,
        ts("2026-10-22T09:00:00Z"),
    );

    assert_eq!(total, SignedDuration::ZERO);
}
