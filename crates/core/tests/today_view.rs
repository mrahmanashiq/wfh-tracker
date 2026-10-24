#![allow(clippy::unwrap_used)] // test code may unwrap (SPEC: Code Style)

use jiff::Timestamp;
use jiff::civil::{Date, date};
use jiff::tz::TimeZone;
use wt_core::{Id, Meta, Task, TaskTree, display_order, shows_tag, today_view};

fn dhaka() -> TimeZone {
    TimeZone::get("Asia/Dhaka").unwrap()
}

fn ts(s: &str) -> Timestamp {
    s.parse().unwrap()
}

fn task(title: &str, parent: Option<&Task>, done_at: Option<&str>) -> Task {
    let created = ts("2026-10-18T03:00:00Z");
    Task {
        meta: Meta {
            id: Id::new(),
            created_at: created,
            updated_at: created,
            deleted_at: None,
        },
        parent_id: parent.map(|p| p.meta.id),
        title: title.to_owned(),
        category_id: Id::new(),
        done_at: done_at.map(ts),
        break_interval_min: None,
    }
}

fn titles(view: &[TaskTree]) -> Vec<(String, Vec<String>)> {
    view.iter()
        .map(|t| {
            (
                t.task.title.clone(),
                t.children.iter().map(|c| c.task.title.clone()).collect(),
            )
        })
        .collect()
}

fn view(tasks: &[Task], day: Date) -> Vec<TaskTree> {
    today_view(tasks, day, &dhaka())
}

#[test]
fn open_tasks_carry_over_until_done() {
    let old = task("Created three days ago", None, None);

    assert_eq!(
        titles(&view(&[old], date(2026, 10, 21))),
        [("Created three days ago".to_owned(), vec![])]
    );
}

#[test]
fn a_task_done_late_in_the_local_evening_shows_that_day_only() {
    // 17:50 UTC is 23:50 in Dhaka on the 21st.
    let late = task("Late finish", None, Some("2026-10-21T17:50:00Z"));
    let tasks = [late];

    assert_eq!(view(&tasks, date(2026, 10, 21)).len(), 1);
    assert!(view(&tasks, date(2026, 10, 22)).is_empty());
}

#[test]
fn a_task_done_just_after_local_midnight_belongs_to_the_new_local_day() {
    // 18:30 UTC on the 21st is 00:30 on the 22nd in Dhaka.
    let early = task("Early finish", None, Some("2026-10-21T18:30:00Z"));
    let tasks = [early];

    assert!(view(&tasks, date(2026, 10, 21)).is_empty());
    assert_eq!(view(&tasks, date(2026, 10, 22)).len(), 1);
}

#[test]
fn progress_counts_every_subtask_but_shows_only_todays() {
    let parent = task("Checkout", None, None);
    let earlier = task("Schema", Some(&parent), Some("2026-10-20T06:00:00Z"));
    let today = task("Validation", Some(&parent), Some("2026-10-21T06:00:00Z"));
    let open = task("API", Some(&parent), None);

    let v = view(&[parent, earlier, today, open], date(2026, 10, 21));

    assert_eq!(
        titles(&v),
        [(
            "Checkout".to_owned(),
            vec!["Validation".to_owned(), "API".to_owned()]
        )]
    );
    assert_eq!(v[0].progress, Some((2, 3)));
}

#[test]
fn a_task_without_subtasks_has_no_progress() {
    let solo = task("Read the Rust book", None, None);

    assert_eq!(view(&[solo], date(2026, 10, 21))[0].progress, None);
}

#[test]
fn a_parent_done_earlier_still_shows_while_a_subtask_is_open() {
    let parent = task("Checkout", None, Some("2026-10-19T06:00:00Z"));
    let open = task("Follow-up fix", Some(&parent), None);

    assert_eq!(
        titles(&view(&[parent, open], date(2026, 10, 21))),
        [("Checkout".to_owned(), vec!["Follow-up fix".to_owned()])]
    );
}

#[test]
fn tasks_and_subtasks_keep_creation_order() {
    let first = task("First", None, None);
    let second = task("Second", None, None);
    let a = task("a", Some(&first), None);
    let b = task("b", Some(&first), None);

    let v = view(&[b, second, a, first], date(2026, 10, 21));

    assert_eq!(
        titles(&v),
        [
            ("First".to_owned(), vec!["a".to_owned(), "b".to_owned()]),
            ("Second".to_owned(), vec![])
        ]
    );
}

#[test]
fn a_third_level_shows_under_its_sub_task_and_keeps_the_chain_visible() {
    let checkout = task("Checkout", None, Some("2026-10-19T06:00:00Z"));
    let api = task("API", Some(&checkout), Some("2026-10-19T06:00:00Z"));
    let auth = task("Auth header", Some(&api), None);

    let v = view(&[checkout, api, auth], date(2026, 10, 21));

    assert_eq!(v.len(), 1);
    assert_eq!(v[0].children[0].task.title, "API");
    assert_eq!(v[0].children[0].children[0].task.title, "Auth header");
    assert_eq!(v[0].children[0].progress, Some((0, 1)));
}

#[test]
fn display_order_sinks_done_tasks_below_open_ones_at_every_level() {
    let day = date(2026, 10, 21);
    let done = Some("2026-10-21T05:00:00Z");
    let parent = task("Checkout", None, None);
    let validation = task("Validation", Some(&parent), done);
    let api = task("API", Some(&parent), None);
    let emails = task("Emails", Some(&parent), done);
    let docs = task("Docs", Some(&parent), None);
    let header = task("Header", Some(&api), done);
    let retry = task("Retry", Some(&api), None);
    let tasks = [parent, validation, api, emails, docs, header, retry];

    let ordered = display_order(view(&tasks, day));

    assert_eq!(
        titles(&ordered),
        [(
            "Checkout".to_owned(),
            vec![
                "API".to_owned(),
                "Docs".to_owned(),
                "Validation".to_owned(),
                "Emails".to_owned(),
            ]
        )]
    );
    let api = &ordered[0].children[0];
    let checklist: Vec<_> = api.children.iter().map(|c| c.task.title.as_str()).collect();
    assert_eq!(checklist, ["Retry", "Header"]);
}

#[test]
fn display_order_puts_loose_to_dos_before_projects_and_done_ones_last() {
    let day = date(2026, 10, 21);
    let project = task("Project", None, None);
    let step = task("Step", Some(&project), None);
    let finished = task("Finished", None, Some("2026-10-21T05:00:00Z"));
    let loose = task("Loose", None, None);
    let tasks = [project, step, finished, loose];

    let order: Vec<_> = display_order(view(&tasks, day))
        .into_iter()
        .map(|t| t.task.title)
        .collect();

    assert_eq!(order, ["Loose", "Finished", "Project"]);
}

#[test]
fn display_order_does_not_change_the_tasks() {
    let day = date(2026, 10, 21);
    let a = task("A", None, Some("2026-10-21T05:00:00Z"));
    let b = task("B", None, None);
    let tasks = [a, b];
    let before = view(&tasks, day);

    let mut after = display_order(before.clone());
    after.sort_by_key(|t| t.task.meta.id);

    assert_eq!(after, before);
}

#[test]
fn a_tag_shows_on_top_level_rows_and_only_below_when_the_category_differs() {
    let parent = task("Checkout", None, None);
    let mut same = task("API", Some(&parent), None);
    same.category_id = parent.category_id;
    let other = task("Read the RFC", Some(&parent), None);

    assert!(shows_tag(&parent, None));
    assert!(!shows_tag(&same, Some(&parent)));
    assert!(shows_tag(&other, Some(&parent)));
}
