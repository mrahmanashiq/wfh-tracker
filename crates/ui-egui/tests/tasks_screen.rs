#![allow(clippy::unwrap_used)] // test code may unwrap (SPEC: Code Style)

use std::sync::Arc;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use jiff::Timestamp;
use wt_core::{CategoryRepo, FixedClock, SessionRepo, TaskRepo};
use wt_storage::SqliteStore;
use wt_ui_egui::TasksScreen;

struct State {
    store: SqliteStore,
    clock: Arc<FixedClock>,
    screen: TasksScreen,
}

fn harness() -> Harness<'static, State> {
    let clock = Arc::new(FixedClock::utc(
        "2026-10-22T04:00:00Z".parse::<Timestamp>().unwrap(),
    ));
    let store = SqliteStore::open_in_memory(clock.clone()).unwrap();
    store.seed_defaults().unwrap();
    Harness::new_ui_state(
        |ui, s: &mut State| s.screen.ui(ui, &s.store, s.clock.as_ref()),
        State {
            store,
            clock,
            screen: TasksScreen::default(),
        },
    )
}

fn add_task(h: &mut Harness<'_, State>, title: &str) {
    h.get_by_label("New task title")
        .set_text_value_accesskit(title);
    h.run();
    h.get_by_label("Add task").click();
    h.run();
}

/// Points at the row named `title`; only the hovered row shows its actions.
fn hover_row(h: &mut Harness<'_, State>, title: &str) {
    h.get_by_label(title).hover();
    h.run();
}

/// Opens the "More" menu of the row named `row` and picks `item`.
fn row_menu(h: &mut Harness<'_, State>, row: &str, item: &str) {
    hover_row(h, row);
    h.get_by_label("More").click();
    h.run();
    h.get_by_label(item).click();
    h.run();
}

/// Adds a sub-task under the top-level task named `parent`.
fn add_subtask(h: &mut Harness<'_, State>, parent: &str, title: &str) {
    row_menu(h, parent, "Add sub-task");
    h.get_by_label("Sub-task title")
        .set_text_value_accesskit(title);
    h.run();
    h.get_by_label("Add").click();
    h.run();
}

fn checkout_with_two_subtasks() -> Harness<'static, State> {
    let mut h = harness();
    add_task(&mut h, "Checkout");
    add_subtask(&mut h, "Checkout", "Validation");
    add_subtask(&mut h, "Checkout", "API");
    h
}

#[test]
fn a_task_with_subtasks_shows_its_progress() {
    let h = checkout_with_two_subtasks();

    h.get_by_label("Checkout");
    h.get_by_label("Validation");
    h.get_by_label("API");
    h.get_by_label("0/2");
}

#[test]
fn ticking_every_subtask_completes_the_parent() {
    let mut h = checkout_with_two_subtasks();

    h.get_by_label("Validation").click();
    h.run();
    h.get_by_label("1/2");

    h.get_by_label("API").click();
    h.run();
    h.get_by_label("2/2");

    let tasks = h.state().store.list_live().unwrap();
    let checkout = tasks.iter().find(|t| t.title == "Checkout").unwrap();
    assert!(checkout.is_done());
}

#[test]
fn deleting_a_task_removes_it_and_its_subtasks() {
    let mut h = checkout_with_two_subtasks();
    add_task(&mut h, "Other");

    row_menu(&mut h, "Checkout", "Delete");
    h.get_by_label("Confirm delete").click();
    h.run();

    assert!(h.query_by_label("Checkout").is_none());
    assert!(h.query_by_label("Validation").is_none());
    h.get_by_label("Other");
}

#[test]
fn a_blank_title_is_refused_with_a_message() {
    let mut h = harness();

    add_task(&mut h, "   ");

    h.get_by_label("a task needs a title");
    assert!(h.state().store.list_live().unwrap().is_empty());
}

#[test]
fn start_and_stop_run_a_timer_on_a_task() {
    let mut h = checkout_with_two_subtasks();

    hover_row(&mut h, "API");
    h.get_by_label("Start").click();
    h.run();
    let running = h.state().store.running().unwrap().unwrap();
    let api = h
        .state()
        .store
        .list_live()
        .unwrap()
        .into_iter()
        .find(|t| t.title == "API")
        .unwrap();
    assert_eq!(running.task_id, Some(api.meta.id));

    h.get_by_label("Stop").click();
    h.run();
    assert!(h.state().store.running().unwrap().is_none());
}
