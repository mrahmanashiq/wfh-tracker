#![allow(clippy::unwrap_used)] // test code may unwrap (SPEC: Code Style)

use jiff::civil::date;
use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp};
use wt_core::{
    CategoryKind, CategoryTime, DayOffice, DistractionEvent, DistractionRule, Id, MatchOn, Meta,
    RuleTime, Session, Settings, TimelineSegment, WatchContext, category_totals, day_timeline,
    distraction_totals, kind_time, office_by_day, timeline_window,
};

// 2026-10-22 is a Thursday. The week from Sunday 18 Oct runs to Sat 24 Oct.

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

fn session(
    task: Option<Id>,
    category: Id,
    kind: CategoryKind,
    from: &str,
    to: Option<&str>,
) -> Session {
    Session {
        meta: meta(),
        task_id: task,
        category_id: category,
        kind,
        started_at: ts(from),
        ended_at: to.map(ts),
    }
}

fn mins(m: i64) -> SignedDuration {
    SignedDuration::from_mins(m)
}

#[test]
fn the_timeline_has_one_segment_per_session_clipped_to_the_day() {
    let (task, office, brk) = (Id::new(), Id::new(), Id::new());
    let sessions = [
        // Crosses midnight into Thursday.
        session(
            Some(task),
            office,
            CategoryKind::Office,
            "2026-10-21T23:00:00Z",
            Some("2026-10-22T01:30:00Z"),
        ),
        session(
            None,
            brk,
            CategoryKind::Break,
            "2026-10-22T01:30:00Z",
            Some("2026-10-22T01:35:00Z"),
        ),
        // Still running at 03:10.
        session(
            Some(task),
            office,
            CategoryKind::Office,
            "2026-10-22T01:35:00Z",
            None,
        ),
    ];
    let now = ts("2026-10-22T03:10:45Z");

    let thursday = day_timeline(&sessions, date(2026, 10, 22), &TimeZone::UTC, now);
    let segment = |kind, from_min, to_min, running| TimelineSegment {
        task_id: if kind == CategoryKind::Break {
            None
        } else {
            Some(task)
        },
        category_id: if kind == CategoryKind::Break {
            brk
        } else {
            office
        },
        kind,
        running,
        from_min,
        to_min,
    };
    assert_eq!(
        thursday,
        vec![
            segment(CategoryKind::Office, 0, 90, false),
            segment(CategoryKind::Break, 90, 95, false),
            segment(CategoryKind::Office, 95, 190, true),
        ]
    );

    let wednesday = day_timeline(&sessions, date(2026, 10, 21), &TimeZone::UTC, now);
    assert_eq!(
        wednesday,
        vec![segment(CategoryKind::Office, 23 * 60, 24 * 60, false)]
    );
}

#[test]
fn the_timeline_window_covers_the_tracked_hours_and_at_least_eight() {
    let seg = |from_min, to_min| TimelineSegment {
        task_id: None,
        category_id: Id::new(),
        kind: CategoryKind::Office,
        running: false,
        from_min,
        to_min,
    };

    assert_eq!(timeline_window(&[]), (9 * 60, 17 * 60));
    // 09:15-10:30 -> 09:00, widened to 8 hours.
    assert_eq!(timeline_window(&[seg(555, 630)]), (9 * 60, 17 * 60));
    // 06:40-20:05 -> 06:00-21:00.
    assert_eq!(
        timeline_window(&[seg(400, 500), seg(1100, 1205)]),
        (6 * 60, 21 * 60)
    );
    // Late in the day the window grows earlier instead.
    assert_eq!(
        timeline_window(&[seg(22 * 60, 23 * 60 + 10)]),
        (16 * 60, 24 * 60)
    );
}

#[test]
fn office_time_per_day_follows_the_week_start_with_targets_and_overtime() {
    let (office, meeting, learning) = (Id::new(), Id::new(), Id::new());
    let sessions = [
        // Sunday: 7h 30m, under the target.
        session(
            None,
            office,
            CategoryKind::Office,
            "2026-10-18T02:00:00Z",
            Some("2026-10-18T09:30:00Z"),
        ),
        // Monday: 9h 30m of office and meetings: 30m past the 9h ceiling.
        session(
            None,
            office,
            CategoryKind::Office,
            "2026-10-19T01:00:00Z",
            Some("2026-10-19T09:00:00Z"),
        ),
        session(
            None,
            meeting,
            CategoryKind::Meeting,
            "2026-10-19T09:00:00Z",
            Some("2026-10-19T10:30:00Z"),
        ),
        // Learning never counts as office time.
        session(
            None,
            learning,
            CategoryKind::Learning,
            "2026-10-20T15:00:00Z",
            Some("2026-10-20T16:00:00Z"),
        ),
        // Friday, a day off: all 2h are overtime. Running until now.
        session(
            None,
            office,
            CategoryKind::Office,
            "2026-10-23T06:00:00Z",
            None,
        ),
    ];
    let now = ts("2026-10-23T08:00:00Z");

    let week = office_by_day(
        &sessions,
        date(2026, 10, 18),
        &TimeZone::UTC,
        now,
        &Settings::default(),
    );

    let day = |d, office: i64, target: Option<i64>, overtime: i64| DayOffice {
        date: date(2026, 10, d),
        office: mins(office),
        target: target.map(mins),
        overtime: mins(overtime),
    };
    assert_eq!(
        week,
        vec![
            day(18, 450, Some(480), 0),
            day(19, 570, Some(480), 30),
            day(20, 0, Some(480), 0),
            day(21, 0, Some(480), 0),
            day(22, 0, Some(480), 0),
            day(23, 120, None, 120),
            day(24, 0, None, 0),
        ]
    );
}

#[test]
fn category_totals_count_the_range_only_without_breaks_most_first() {
    let (office, family, learning, brk) = (Id::new(), Id::new(), Id::new(), Id::new());
    let sessions = [
        // Saturday before the week: 1h of it counts on Sunday after midnight.
        session(
            None,
            family,
            CategoryKind::Personal,
            "2026-10-17T23:00:00Z",
            Some("2026-10-18T01:00:00Z"),
        ),
        session(
            None,
            learning,
            CategoryKind::Learning,
            "2026-10-20T19:00:00Z",
            Some("2026-10-20T21:20:00Z"),
        ),
        session(
            None,
            brk,
            CategoryKind::Break,
            "2026-10-20T21:20:00Z",
            Some("2026-10-20T21:30:00Z"),
        ),
        session(
            None,
            office,
            CategoryKind::Office,
            "2026-10-22T02:00:00Z",
            None,
        ),
        // Next week: not counted.
        session(
            None,
            learning,
            CategoryKind::Learning,
            "2026-10-25T19:00:00Z",
            Some("2026-10-25T20:00:00Z"),
        ),
    ];
    let now = ts("2026-10-22T05:30:00Z");

    let totals = category_totals(
        &sessions,
        date(2026, 10, 18),
        date(2026, 10, 25),
        &TimeZone::UTC,
        now,
    );

    let time = |category_id, m| CategoryTime {
        category_id,
        time: mins(m),
    };
    assert_eq!(
        totals,
        vec![time(office, 210), time(learning, 140), time(family, 60)]
    );
}

#[test]
fn kind_time_adds_up_the_days_sessions_of_the_given_kinds() {
    let (learning, family, custom) = (Id::new(), Id::new(), Id::new());
    let sessions = [
        session(
            None,
            learning,
            CategoryKind::Learning,
            "2026-10-22T19:00:00Z",
            Some("2026-10-22T19:50:00Z"),
        ),
        session(
            None,
            family,
            CategoryKind::Personal,
            "2026-10-22T20:00:00Z",
            Some("2026-10-22T20:30:00Z"),
        ),
        session(
            None,
            custom,
            CategoryKind::Custom,
            "2026-10-22T23:30:00Z",
            None,
        ),
    ];
    let now = ts("2026-10-23T00:20:00Z");
    let day = date(2026, 10, 22);
    let tz = TimeZone::UTC;

    assert_eq!(
        kind_time(&sessions, day, &tz, now, |k| k == CategoryKind::Learning),
        mins(50)
    );
    assert_eq!(
        kind_time(&sessions, day, &tz, now, |k| matches!(
            k,
            CategoryKind::Personal | CategoryKind::Custom
        )),
        mins(60)
    );
}

#[test]
fn distraction_totals_list_time_per_rule_in_rule_order() {
    let rule = |pattern: &str| DistractionRule {
        meta: meta(),
        pattern: pattern.to_owned(),
        match_on: MatchOn::Title,
        enabled: true,
    };
    let (facebook, reels) = (rule("Facebook"), rule("Reels"));
    let event = |rule_id, context, from: &str, to: Option<&str>| DistractionEvent {
        meta: meta(),
        rule_id,
        context,
        started_at: ts(from),
        ended_at: to.map(ts),
    };
    let deleted = Id::new();
    let events = [
        event(
            reels.meta.id,
            WatchContext::Learning,
            "2026-10-19T19:00:00Z",
            Some("2026-10-19T19:10:00Z"),
        ),
        event(
            facebook.meta.id,
            WatchContext::Office,
            "2026-10-20T03:00:00Z",
            Some("2026-10-20T03:20:00Z"),
        ),
        // Open until now: 5 more minutes.
        event(
            facebook.meta.id,
            WatchContext::Learning,
            "2026-10-22T05:25:00Z",
            None,
        ),
        event(
            deleted,
            WatchContext::Office,
            "2026-10-21T03:00:00Z",
            Some("2026-10-21T03:03:00Z"),
        ),
        // Reels for 30 s on another day: in the total, not as a line of its own.
        event(
            reels.meta.id,
            WatchContext::Office,
            "2026-10-22T04:00:00Z",
            Some("2026-10-22T04:00:30Z"),
        ),
        // The week before: not counted.
        event(
            facebook.meta.id,
            WatchContext::Office,
            "2026-10-17T03:00:00Z",
            Some("2026-10-17T04:00:00Z"),
        ),
    ];
    let now = ts("2026-10-22T05:30:00Z");

    let totals = distraction_totals(
        &events,
        &[facebook, reels],
        date(2026, 10, 18),
        date(2026, 10, 25),
        &TimeZone::UTC,
        now,
    );

    assert_eq!(totals.total, SignedDuration::from_secs(38 * 60 + 30));
    let line = |name: &str, time| RuleTime {
        name: name.to_owned(),
        time,
    };
    assert_eq!(
        totals.rules,
        vec![
            line("Facebook", mins(25)),
            line("Reels", SignedDuration::from_secs(10 * 60 + 30)),
            line("(deleted rule)", mins(3)),
        ]
    );
}
