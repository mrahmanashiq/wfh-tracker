#![allow(clippy::unwrap_used)] // test code may unwrap (SPEC: Code Style)

use jiff::Timestamp;
use jiff::civil::{Date, date};
use jiff::tz::TimeZone;
use wt_core::{
    Category, CategoryKind, Id, Meta, ReportInput, Rgb, Session, Settings, Task, my_time_report,
    office_report,
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

fn category(name: &str, kind: CategoryKind) -> Category {
    Category {
        meta: meta(),
        name: name.to_owned(),
        kind,
        color: Rgb(0, 0, 0),
    }
}

/// A day's worth of data, built up by the tests.
struct Day {
    office: Category,
    meeting: Category,
    learning: Category,
    family: Category,
    social: Category,
    brk: Category,
    tasks: Vec<Task>,
    sessions: Vec<Session>,
    all_categories: Vec<Category>,
    settings: Settings,
}

impl Day {
    fn new() -> Self {
        Self {
            office: category("Office", CategoryKind::Office),
            meeting: category("Meeting", CategoryKind::Meeting),
            learning: category("Learning", CategoryKind::Learning),
            family: category("Family", CategoryKind::Personal),
            social: category("Social media", CategoryKind::Personal),
            brk: category("Break", CategoryKind::Break),
            tasks: Vec::new(),
            sessions: Vec::new(),
            all_categories: Vec::new(),
            settings: Settings::default(),
        }
        .with_category_list()
    }

    fn with_category_list(mut self) -> Self {
        self.all_categories = [
            &self.office,
            &self.meeting,
            &self.learning,
            &self.family,
            &self.social,
            &self.brk,
        ]
        .into_iter()
        .cloned()
        .collect();
        self
    }

    fn task(
        &mut self,
        title: &str,
        cat: &Category,
        parent: Option<Id>,
        done_at: Option<&str>,
    ) -> Id {
        let task = Task {
            meta: meta(),
            parent_id: parent,
            title: title.to_owned(),
            category_id: cat.meta.id,
            done_at: done_at.map(ts),
            break_interval_min: None,
        };
        let id = task.meta.id;
        self.tasks.push(task);
        id
    }

    fn session(&mut self, task: Option<Id>, cat: &Category, from: &str, to: &str) {
        self.sessions.push(Session {
            meta: meta(),
            task_id: task,
            category_id: cat.meta.id,
            kind: cat.kind,
            started_at: ts(from),
            ended_at: Some(ts(to)),
        });
    }

    fn office_report(&self, day: Date, tz: &TimeZone) -> String {
        office_report(&self.input(day, tz))
    }

    fn my_time_report(&self, day: Date, tz: &TimeZone) -> String {
        my_time_report(&self.input(day, tz))
    }

    fn input<'a>(&'a self, day: Date, tz: &'a TimeZone) -> ReportInput<'a> {
        ReportInput {
            day,
            tz,
            now: ts("2026-10-24T00:00:00Z"),
            sessions: &self.sessions,
            tasks: &self.tasks,
            categories: &self.all_categories,
            settings: &self.settings,
        }
    }
}

const THURSDAY: Date = date(2026, 10, 22);
const FRIDAY: Date = date(2026, 10, 23);

#[test]
fn the_office_report_matches_the_spec_example() {
    let mut d = Day::new();
    let (office, meeting, learning, brk) = (
        d.office.clone(),
        d.meeting.clone(),
        d.learning.clone(),
        d.brk.clone(),
    );
    let checkout = d.task("Checkout redesign", &office, None, None);
    let validation = d.task(
        "Payment form validation",
        &office,
        Some(checkout),
        Some("2026-10-22T05:10:00Z"),
    );
    let api = d.task("Stripe API integration", &office, Some(checkout), None);
    d.task("Order summary emails", &office, Some(checkout), None); // untouched: left out
    let review = d.task(
        "Review PR #482",
        &office,
        None,
        Some("2026-10-22T09:55:00Z"),
    );
    d.task("Old idea", &office, None, None); // untouched: left out
    let book = d.task("Rust async book", &learning, None, None); // not office

    d.session(
        Some(validation),
        &office,
        "2026-10-22T03:00:00Z",
        "2026-10-22T05:10:00Z",
    );
    d.session(None, &brk, "2026-10-22T05:10:00Z", "2026-10-22T05:20:00Z");
    d.session(
        Some(api),
        &office,
        "2026-10-22T05:20:00Z",
        "2026-10-22T08:25:00Z",
    );
    d.session(
        None,
        &meeting,
        "2026-10-22T08:25:00Z",
        "2026-10-22T09:05:00Z",
    );
    d.session(
        Some(review),
        &office,
        "2026-10-22T09:05:00Z",
        "2026-10-22T09:55:00Z",
    );
    d.session(
        None,
        &meeting,
        "2026-10-22T09:55:00Z",
        "2026-10-22T10:15:00Z",
    );
    d.session(
        None,
        &meeting,
        "2026-10-22T10:15:00Z",
        "2026-10-22T10:35:00Z",
    );
    d.session(
        None,
        &office,
        "2026-10-22T10:35:00Z",
        "2026-10-22T10:55:00Z",
    );
    d.session(
        Some(book),
        &learning,
        "2026-10-22T13:00:00Z",
        "2026-10-22T13:50:00Z",
    );

    assert_eq!(
        d.office_report(THURSDAY, &TimeZone::UTC),
        "Office report — Thu 22 Oct 2026 — 7h 45m (target 8h)\n\
         \n\
         Checkout redesign — 5h 15m\n  \
         ✅ Payment form validation — 2h 10m\n  \
         ⏳ Stripe API integration — 3h 05m\n\
         ✅ Review PR #482 — 50m\n\
         Other office work — 20m\n\
         Meetings — 3 (1h 20m)\n\
         Overtime — none"
    );
}

#[test]
fn overtime_and_time_on_a_deleted_task_are_reported() {
    let mut d = Day::new();
    let office = d.office.clone();
    // A task id that no longer exists in the task list.
    d.session(
        Some(Id::new()),
        &office,
        "2026-10-22T01:00:00Z",
        "2026-10-22T10:30:00Z",
    );

    assert_eq!(
        d.office_report(THURSDAY, &TimeZone::UTC),
        "Office report — Thu 22 Oct 2026 — 9h 30m (target 8h)\n\
         \n\
         (deleted task) — 9h 30m\n\
         Meetings — none\n\
         Overtime — 30m"
    );
}

#[test]
fn on_a_day_off_all_office_time_is_overtime() {
    let mut d = Day::new();
    let office = d.office.clone();
    let hotfix = d.task("Hotfix", &office, None, None);
    d.session(
        Some(hotfix),
        &office,
        "2026-10-23T05:00:00Z",
        "2026-10-23T08:00:00Z",
    );

    assert_eq!(
        d.office_report(FRIDAY, &TimeZone::UTC),
        "Office report — Fri 23 Oct 2026 — 3h 00m (day off)\n\
         \n\
         ⏳ Hotfix — 3h 00m\n\
         Meetings — none\n\
         Overtime — 3h 00m"
    );
}

#[test]
fn a_session_across_local_midnight_counts_only_its_part_of_the_day() {
    let dhaka = TimeZone::get("Asia/Dhaka").unwrap();
    let mut d = Day::new();
    let office = d.office.clone();
    let late = d.task("Late deploy", &office, None, None);
    // 17:00–19:00 UTC is 23:00–01:00 in Dhaka.
    d.session(
        Some(late),
        &office,
        "2026-10-21T17:00:00Z",
        "2026-10-21T19:00:00Z",
    );

    assert_eq!(
        d.office_report(THURSDAY, &dhaka),
        "Office report — Thu 22 Oct 2026 — 1h 00m (target 8h)\n\
         \n\
         ⏳ Late deploy — 1h 00m\n\
         Meetings — none\n\
         Overtime — none"
    );
}

#[test]
fn my_time_shows_learning_then_personal_time_by_category() {
    let mut d = Day::new();
    let (office, learning, family, social) = (
        d.office.clone(),
        d.learning.clone(),
        d.family.clone(),
        d.social.clone(),
    );
    let book = d.task(
        "Rust async book, chapter 4",
        &learning,
        None,
        Some("2026-10-22T13:50:00Z"),
    );
    let call = d.task("Call parents", &family, None, None);
    let work = d.task("Checkout", &office, None, None);
    d.session(
        Some(work),
        &office,
        "2026-10-22T03:00:00Z",
        "2026-10-22T04:00:00Z",
    ); // not mine
    d.session(
        Some(book),
        &learning,
        "2026-10-22T13:00:00Z",
        "2026-10-22T13:50:00Z",
    );
    d.session(
        None,
        &learning,
        "2026-10-22T14:00:00Z",
        "2026-10-22T14:30:00Z",
    );
    d.session(
        Some(call),
        &family,
        "2026-10-22T15:00:00Z",
        "2026-10-22T15:30:00Z",
    );
    d.session(
        None,
        &family,
        "2026-10-22T16:00:00Z",
        "2026-10-22T17:00:00Z",
    );
    d.session(
        None,
        &social,
        "2026-10-22T17:00:00Z",
        "2026-10-22T17:40:00Z",
    );

    assert_eq!(
        d.my_time_report(THURSDAY, &TimeZone::UTC),
        "My time — Thu 22 Oct 2026\n\
         \n\
         Learning — 1h 20m\n  \
         ✅ Rust async book, chapter 4 — 50m\n  \
         Learning (no task) — 30m\n\
         \n\
         Personal — 2h 10m\n  \
         Family — 1h 30m\n  \
         Social media — 40m"
    );
}

#[test]
fn an_empty_my_time_report_says_so() {
    let d = Day::new();

    assert_eq!(
        d.my_time_report(THURSDAY, &TimeZone::UTC),
        "My time — Thu 22 Oct 2026\n\nNothing tracked for yourself this day."
    );
}

#[test]
fn a_task_finished_after_the_report_day_still_shows_as_open_that_day() {
    let mut d = Day::new();
    let office = d.office.clone();
    let api = d.task("API", &office, None, Some("2026-10-23T09:00:00Z"));
    d.session(
        Some(api),
        &office,
        "2026-10-22T03:00:00Z",
        "2026-10-22T04:00:00Z",
    );

    assert!(
        d.office_report(THURSDAY, &TimeZone::UTC)
            .contains("⏳ API — 1h 00m")
    );
}

#[test]
fn lines_follow_the_session_kind_so_they_add_up_to_the_totals() {
    let mut d = Day::new();
    let (office, learning) = (d.office.clone(), d.learning.clone());
    // Tracked as learning, then the task was moved to an office category.
    let task = d.task("Moved task", &office, None, None);
    d.sessions.push(Session {
        meta: meta(),
        task_id: Some(task),
        category_id: learning.meta.id,
        kind: CategoryKind::Learning,
        started_at: ts("2026-10-22T13:00:00Z"),
        ended_at: Some(ts("2026-10-22T13:40:00Z")),
    });

    let office_text = d.office_report(THURSDAY, &TimeZone::UTC);
    assert!(!office_text.contains("Moved task"), "{office_text}");
    assert!(
        d.my_time_report(THURSDAY, &TimeZone::UTC)
            .contains("Learning — 0h 40m\n  ⏳ Moved task — 40m"),
    );
}

#[test]
fn time_in_a_deleted_category_still_gets_a_line() {
    let mut d = Day::new();
    let gaming = category("Gaming", CategoryKind::Personal); // not in the live list
    d.session(
        None,
        &gaming,
        "2026-10-22T18:00:00Z",
        "2026-10-22T19:00:00Z",
    );

    assert_eq!(
        d.my_time_report(THURSDAY, &TimeZone::UTC),
        "My time — Thu 22 Oct 2026\n\nPersonal — 1h 00m\n  (deleted category) — 1h 00m"
    );
}

#[test]
fn time_under_a_minute_is_left_out_instead_of_showing_zero() {
    let mut d = Day::new();
    let (office, learning) = (d.office.clone(), d.learning.clone());
    let quick = d.task("Quick look", &office, None, None);
    d.session(
        Some(quick),
        &office,
        "2026-10-22T03:00:00Z",
        "2026-10-22T03:00:40Z",
    );
    d.session(
        None,
        &learning,
        "2026-10-22T13:00:00Z",
        "2026-10-22T13:00:30Z",
    );

    assert_eq!(
        d.my_time_report(THURSDAY, &TimeZone::UTC),
        "My time — Thu 22 Oct 2026\n\nNothing tracked for yourself this day."
    );
    let office_text = d.office_report(THURSDAY, &TimeZone::UTC);
    assert!(!office_text.contains("Quick look"), "{office_text}");
}

#[test]
fn a_third_level_is_indented_and_counted_in_every_total_above() {
    let mut d = Day::new();
    let office = d.office.clone();
    let checkout = d.task("Checkout", &office, None, None);
    let api = d.task("API", &office, Some(checkout), None);
    let auth = d.task(
        "Auth header",
        &office,
        Some(api),
        Some("2026-10-22T04:00:00Z"),
    );
    d.session(
        Some(api),
        &office,
        "2026-10-22T03:00:00Z",
        "2026-10-22T03:20:00Z",
    );
    d.session(
        Some(auth),
        &office,
        "2026-10-22T03:30:00Z",
        "2026-10-22T04:00:00Z",
    );

    assert_eq!(
        d.office_report(THURSDAY, &TimeZone::UTC),
        "Office report — Thu 22 Oct 2026 — 0h 50m (target 8h)\n\
         \n\
         Checkout — 50m\n  \
         API — 50m\n    \
         ✅ Auth header — 30m\n\
         Meetings — none\n\
         Overtime — none"
    );
}
