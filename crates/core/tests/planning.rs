#![allow(clippy::unwrap_used)] // test code may unwrap (SPEC: Code Style)

use jiff::civil::{Weekday, date};
use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp};
use wt_core::{
    Block, CategoryKind, Id, Meta, Notice, Period, Reminders, Session, Settings, Target, Tick,
    Weekdays, active_block, period_range, target_progress,
};

fn ts(s: &str) -> Timestamp {
    s.parse().unwrap()
}

fn meta() -> Meta {
    let t = ts("2026-10-01T00:00:00Z");
    Meta {
        id: Id::new(),
        created_at: t,
        updated_at: t,
        deleted_at: None,
    }
}

fn session(category: Id, kind: CategoryKind, from: &str, to: Option<&str>) -> Session {
    Session {
        meta: meta(),
        task_id: None,
        category_id: category,
        kind,
        started_at: ts(from),
        ended_at: to.map(ts),
    }
}

fn block(category: Id, days: &[Weekday], start: &str, end: &str) -> Block {
    let minutes = |hm: &str| {
        let (h, m) = hm.split_once(':').unwrap();
        h.parse::<u16>().unwrap() * 60 + m.parse::<u16>().unwrap()
    };
    Block {
        meta: meta(),
        category_id: category,
        weekdays: Weekdays::from_days(days),
        start_min: minutes(start),
        end_min: minutes(end),
        label: String::new(),
    }
}

const SUN_TO_THU: [Weekday; 5] = [
    Weekday::Sunday,
    Weekday::Monday,
    Weekday::Tuesday,
    Weekday::Wednesday,
    Weekday::Thursday,
];

// 2026-10-22 is a Thursday.

#[test]
fn a_week_starts_on_the_configured_day() {
    let wednesday = date(2026, 10, 21);

    assert_eq!(
        period_range(wednesday, Period::Week, Weekday::Sunday),
        (date(2026, 10, 18), date(2026, 10, 25))
    );
    assert_eq!(
        period_range(wednesday, Period::Week, Weekday::Monday),
        (date(2026, 10, 19), date(2026, 10, 26))
    );
    assert_eq!(
        period_range(date(2026, 10, 25), Period::Week, Weekday::Sunday),
        (date(2026, 10, 25), date(2026, 11, 1))
    );
}

#[test]
fn a_month_is_the_calendar_month() {
    assert_eq!(
        period_range(date(2026, 10, 22), Period::Month, Weekday::Sunday),
        (date(2026, 10, 1), date(2026, 11, 1))
    );
    assert_eq!(
        period_range(date(2026, 12, 31), Period::Month, Weekday::Sunday),
        (date(2026, 12, 1), date(2027, 1, 1))
    );
}

#[test]
fn target_progress_counts_this_periods_time_in_that_category_only() {
    let (learning, family) = (Id::new(), Id::new());
    let target = Target {
        meta: meta(),
        category_id: learning,
        period: Period::Week,
        minutes: 5 * 60,
    };
    let sessions = [
        // Last week (week starts Sun 18 Oct): not counted.
        session(
            learning,
            CategoryKind::Learning,
            "2026-10-17T13:00:00Z",
            Some("2026-10-17T16:00:00Z"),
        ),
        // This week.
        session(
            learning,
            CategoryKind::Learning,
            "2026-10-19T13:00:00Z",
            Some("2026-10-19T14:00:00Z"),
        ),
        session(
            family,
            CategoryKind::Personal,
            "2026-10-20T13:00:00Z",
            Some("2026-10-20T15:00:00Z"),
        ),
        // Running now.
        session(
            learning,
            CategoryKind::Learning,
            "2026-10-22T13:00:00Z",
            None,
        ),
    ];

    let done = target_progress(
        &sessions,
        &target,
        date(2026, 10, 22),
        &TimeZone::UTC,
        ts("2026-10-22T13:30:00Z"),
        Weekday::Sunday,
    );

    assert_eq!(done, SignedDuration::from_mins(90));
}

/// Ticks reminders at `at` (UTC) with `blocks`; break and office notices are ignored.
fn block_notices(
    r: &mut Reminders,
    sessions: &[Session],
    blocks: &[Block],
    at: &str,
) -> Vec<Notice> {
    r.tick(Tick {
        sessions,
        now: ts(at),
        tz: &TimeZone::UTC,
        settings: &Settings::default(),
        break_interval_min: None,
        blocks,
    })
    .into_iter()
    .filter(|n| matches!(n, Notice::BlockStarted { .. }))
    .collect()
}

#[test]
fn a_block_reminds_once_when_it_starts_on_its_days() {
    let learning = Id::new();
    let blocks = [block(learning, &SUN_TO_THU, "21:00", "22:00")];
    let id = blocks[0].meta.id;
    let mut r = Reminders::default();

    assert_eq!(
        block_notices(&mut r, &[], &blocks, "2026-10-22T20:59:00Z"),
        []
    );
    assert_eq!(
        block_notices(&mut r, &[], &blocks, "2026-10-22T21:00:00Z"),
        [Notice::BlockStarted { block: id }]
    );
    assert_eq!(
        block_notices(&mut r, &[], &blocks, "2026-10-22T21:05:00Z"),
        []
    );
    // Friday is not one of its days.
    assert_eq!(
        block_notices(&mut r, &[], &blocks, "2026-10-23T21:00:00Z"),
        []
    );
    // Sunday is, and it's a new day.
    assert_eq!(
        block_notices(&mut r, &[], &blocks, "2026-10-25T21:00:00Z"),
        [Notice::BlockStarted { block: id }]
    );
}

#[test]
fn no_block_reminder_when_its_timer_is_already_running() {
    let learning = Id::new();
    let blocks = [block(learning, &SUN_TO_THU, "21:00", "22:00")];
    let running = [session(
        learning,
        CategoryKind::Learning,
        "2026-10-22T20:50:00Z",
        None,
    )];
    let mut r = Reminders::default();

    assert_eq!(
        block_notices(&mut r, &running, &blocks, "2026-10-22T21:00:00Z"),
        []
    );
    // Stopping the timer later in the block doesn't bring the reminder back.
    assert_eq!(
        block_notices(&mut r, &[], &blocks, "2026-10-22T21:10:00Z"),
        []
    );
}

#[test]
fn a_block_already_on_when_the_app_starts_reminds_once() {
    let blocks = [block(Id::new(), &SUN_TO_THU, "21:00", "22:00")];
    let id = blocks[0].meta.id;
    let mut r = Reminders::default();

    assert_eq!(
        block_notices(&mut r, &[], &blocks, "2026-10-22T21:30:00Z"),
        [Notice::BlockStarted { block: id }]
    );
    assert_eq!(
        block_notices(&mut r, &[], &blocks, "2026-10-22T22:00:00Z"),
        []
    );
}

#[test]
fn the_active_block_is_the_earliest_starting_one_that_is_on() {
    let (a, b) = (Id::new(), Id::new());
    let blocks = [
        block(a, &SUN_TO_THU, "21:30", "23:00"),
        block(b, &SUN_TO_THU, "21:00", "22:00"),
    ];

    let at = |s: &str| active_block(&blocks, ts(s), &TimeZone::UTC).map(|b| b.category_id);
    assert_eq!(at("2026-10-22T20:59:00Z"), None);
    assert_eq!(at("2026-10-22T21:45:00Z"), Some(b));
    assert_eq!(at("2026-10-22T22:30:00Z"), Some(a));
    assert_eq!(at("2026-10-22T23:00:00Z"), None);
    assert_eq!(at("2026-10-23T21:45:00Z"), None); // Friday
}

#[test]
fn moving_a_block_later_in_the_day_reminds_again_at_its_new_time() {
    let learning = Id::new();
    let mut blocks = [block(learning, &SUN_TO_THU, "21:00", "22:00")];
    let id = blocks[0].meta.id;
    let mut r = Reminders::default();
    assert_eq!(
        block_notices(&mut r, &[], &blocks, "2026-10-22T21:00:00Z"),
        [Notice::BlockStarted { block: id }]
    );

    blocks[0].start_min = 22 * 60 + 30;
    blocks[0].end_min = 23 * 60;

    assert_eq!(
        block_notices(&mut r, &[], &blocks, "2026-10-22T22:30:00Z"),
        [Notice::BlockStarted { block: id }]
    );
}
