#![allow(clippy::unwrap_used)] // test code may unwrap (SPEC: Code Style)

use jiff::civil::{Weekday, date};
use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp};
use wt_core::{
    CategoryKind, Id, Meta, OfficeStatus, Session, Settings, office_status, office_time,
};

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

fn hm(h: i64, m: i64) -> SignedDuration {
    SignedDuration::from_mins(h * 60 + m)
}

#[test]
fn office_and_meetings_count_but_breaks_do_not() {
    let sessions = [
        session(
            CategoryKind::Office,
            "2026-10-22T03:00:00Z",
            Some("2026-10-22T06:00:00Z"),
        ),
        session(
            CategoryKind::Break,
            "2026-10-22T06:00:00Z",
            Some("2026-10-22T06:30:00Z"),
        ),
        session(
            CategoryKind::Meeting,
            "2026-10-22T06:30:00Z",
            Some("2026-10-22T07:30:00Z"),
        ),
        session(
            CategoryKind::Learning,
            "2026-10-22T13:00:00Z",
            Some("2026-10-22T14:00:00Z"),
        ),
    ];

    let total = office_time(
        &sessions,
        date(2026, 10, 22),
        &TimeZone::UTC,
        ts("2026-10-22T20:00:00Z"),
    );

    assert_eq!(total, hm(4, 0));
}

#[test]
fn a_running_session_counts_up_to_now() {
    let sessions = [session(CategoryKind::Office, "2026-10-22T03:00:00Z", None)];

    let total = office_time(
        &sessions,
        date(2026, 10, 22),
        &TimeZone::UTC,
        ts("2026-10-22T04:15:00Z"),
    );

    assert_eq!(total, hm(1, 15));
}

#[test]
fn a_session_across_local_midnight_is_split_between_the_days() {
    let dhaka = TimeZone::get("Asia/Dhaka").unwrap();
    // 17:00–19:00 UTC is 23:00–01:00 in Dhaka.
    let sessions = [session(
        CategoryKind::Office,
        "2026-10-21T17:00:00Z",
        Some("2026-10-21T19:00:00Z"),
    )];
    let now = ts("2026-10-22T06:00:00Z");

    assert_eq!(
        office_time(&sessions, date(2026, 10, 21), &dhaka, now),
        hm(1, 0)
    );
    assert_eq!(
        office_time(&sessions, date(2026, 10, 22), &dhaka, now),
        hm(1, 0)
    );
}

#[test]
fn status_follows_the_8h_target_and_9h_soft_ceiling_on_a_workday() {
    let settings = Settings::default();
    let thursday = Weekday::Thursday;

    assert_eq!(
        office_status(hm(7, 59), &settings, thursday),
        OfficeStatus::UnderTarget
    );
    assert_eq!(
        office_status(hm(8, 0), &settings, thursday),
        OfficeStatus::TargetReached
    );
    assert_eq!(
        office_status(hm(8, 59), &settings, thursday),
        OfficeStatus::TargetReached
    );
    assert_eq!(
        office_status(hm(9, 0), &settings, thursday),
        OfficeStatus::SoftCeiling
    );
    assert_eq!(
        office_status(hm(9, 30), &settings, thursday),
        OfficeStatus::Overtime { minutes: 30 }
    );
}

#[test]
fn on_a_weekend_day_all_office_time_is_overtime() {
    let settings = Settings::default();

    assert_eq!(
        office_status(hm(3, 0), &settings, Weekday::Friday),
        OfficeStatus::Overtime { minutes: 180 }
    );
    assert_eq!(
        office_status(SignedDuration::ZERO, &settings, Weekday::Saturday),
        OfficeStatus::UnderTarget
    );
}
