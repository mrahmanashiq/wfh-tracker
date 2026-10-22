#![allow(clippy::unwrap_used)] // test code may unwrap (SPEC: Code Style)

use jiff::Timestamp;
use jiff::tz::TimeZone;
use wt_core::{CategoryKind, Id, Meta, Notice, Reminders, Session, Settings, Tick};

fn ts(s: &str) -> Timestamp {
    s.parse().unwrap()
}

fn session(kind: CategoryKind, from: &str, to: Option<&str>) -> Session {
    let start = ts(from);
    Session {
        meta: Meta {
            id: Id::new(),
            created_at: start,
            updated_at: start,
            deleted_at: None,
        },
        task_id: None,
        category_id: Id::new(),
        kind,
        started_at: start,
        ended_at: to.map(ts),
    }
}

/// Ticks `r` at `now` with default settings, in UTC.
fn tick(r: &mut Reminders, sessions: &[Session], now: &str) -> Vec<Notice> {
    tick_with(r, sessions, now, None)
}

fn tick_with(
    r: &mut Reminders,
    sessions: &[Session],
    now: &str,
    break_interval_min: Option<u32>,
) -> Vec<Notice> {
    r.tick(Tick {
        sessions,
        now: ts(now),
        tz: &TimeZone::UTC,
        settings: &Settings::default(),
        break_interval_min,
        blocks: &[],
    })
}

/// Drops break notices; continuous work makes breaks due in office-only tests too.
fn office_only(notices: Vec<Notice>) -> Vec<Notice> {
    notices
        .into_iter()
        .filter(|n| matches!(n, Notice::TargetReached | Notice::SoftCeiling))
        .collect()
}

// 2026-10-22 is a Thursday (a workday); 2026-10-23 is a Friday (weekend).

#[test]
fn thresholds_already_passed_when_the_app_starts_do_not_fire() {
    let mut r = Reminders::default();
    let work = [session(CategoryKind::Office, "2026-10-22T00:00:00Z", None)];

    assert_eq!(office_only(tick(&mut r, &work, "2026-10-22T08:30:00Z")), []);
    assert_eq!(office_only(tick(&mut r, &work, "2026-10-22T08:31:00Z")), []);
}

#[test]
fn eight_and_nine_hours_each_fire_once() {
    let mut r = Reminders::default();
    let work = [session(CategoryKind::Office, "2026-10-22T00:00:00Z", None)];
    assert_eq!(office_only(tick(&mut r, &work, "2026-10-22T07:59:00Z")), []);
    assert_eq!(
        office_only(tick(&mut r, &work, "2026-10-22T08:00:00Z")),
        [Notice::TargetReached]
    );
    assert_eq!(office_only(tick(&mut r, &work, "2026-10-22T08:01:00Z")), []);
    assert_eq!(
        office_only(tick(&mut r, &work, "2026-10-22T09:00:00Z")),
        [Notice::SoftCeiling]
    );
    assert_eq!(office_only(tick(&mut r, &work, "2026-10-22T09:30:00Z")), []);
}

#[test]
fn no_office_notices_on_a_weekend_day() {
    let mut r = Reminders::default();
    let work = [session(CategoryKind::Office, "2026-10-23T00:00:00Z", None)];

    tick(&mut r, &work, "2026-10-23T07:59:00Z");
    let notices = tick(&mut r, &work, "2026-10-23T09:01:00Z");

    assert!(!notices.contains(&Notice::TargetReached), "{notices:?}");
    assert!(!notices.contains(&Notice::SoftCeiling), "{notices:?}");
}

#[test]
fn a_break_is_due_after_25_minutes_and_repeats_every_5_until_taken() {
    let mut r = Reminders::default();
    let work = [session(CategoryKind::Office, "2026-10-22T03:00:00Z", None)];

    assert_eq!(tick(&mut r, &work, "2026-10-22T03:24:00Z"), []);
    assert_eq!(
        tick(&mut r, &work, "2026-10-22T03:25:00Z"),
        [Notice::BreakDue]
    );
    assert_eq!(tick(&mut r, &work, "2026-10-22T03:27:00Z"), []);
    assert_eq!(
        tick(&mut r, &work, "2026-10-22T03:30:00Z"),
        [Notice::BreakDue]
    );
}

#[test]
fn break_over_fires_once_when_the_break_length_has_passed() {
    let mut r = Reminders::default();
    let day = [
        session(
            CategoryKind::Office,
            "2026-10-22T03:00:00Z",
            Some("2026-10-22T03:31:00Z"),
        ),
        session(CategoryKind::Break, "2026-10-22T03:31:00Z", None),
    ];

    assert_eq!(tick(&mut r, &day, "2026-10-22T03:35:00Z"), []);
    assert_eq!(
        tick(&mut r, &day, "2026-10-22T03:36:00Z"),
        [Notice::BreakOver]
    );
    assert_eq!(tick(&mut r, &day, "2026-10-22T03:40:00Z"), []);
}

#[test]
fn switching_tasks_with_a_short_gap_keeps_counting_the_same_stretch() {
    let mut r = Reminders::default();
    let day = [
        session(
            CategoryKind::Office,
            "2026-10-22T03:00:00Z",
            Some("2026-10-22T03:15:00Z"),
        ),
        session(CategoryKind::Office, "2026-10-22T03:17:00Z", None),
    ];

    assert_eq!(
        tick(&mut r, &day, "2026-10-22T03:25:00Z"),
        [Notice::BreakDue]
    );
}

#[test]
fn a_gap_as_long_as_a_break_starts_a_new_stretch() {
    let mut r = Reminders::default();
    let day = [
        session(
            CategoryKind::Office,
            "2026-10-22T03:00:00Z",
            Some("2026-10-22T03:15:00Z"),
        ),
        session(CategoryKind::Office, "2026-10-22T03:21:00Z", None),
    ];

    assert_eq!(tick(&mut r, &day, "2026-10-22T03:25:00Z"), []);
    assert_eq!(
        tick(&mut r, &day, "2026-10-22T03:46:00Z"),
        [Notice::BreakDue]
    );
}

#[test]
fn a_tasks_own_break_interval_wins_over_the_global_one() {
    let mut r = Reminders::default();
    let work = [session(CategoryKind::Office, "2026-10-22T03:00:00Z", None)];

    assert_eq!(
        tick_with(&mut r, &work, "2026-10-22T03:25:00Z", Some(50)),
        []
    );
    assert_eq!(
        tick_with(&mut r, &work, "2026-10-22T03:50:00Z", Some(50)),
        [Notice::BreakDue]
    );
}
