#![allow(clippy::unwrap_used)] // test code may unwrap (SPEC: Code Style)

use std::sync::Arc;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use jiff::Timestamp;
use wt_core::{CategoryRepo, FixedClock, SessionRepo, SettingsRepo, TaskRepo};
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
        |ui, s: &mut State| {
            let settings = s.store.load().unwrap();
            s.screen.ui(ui, &s.store, &settings, s.clock.as_ref());
        },
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

/// Hovers the row named `row` and clicks its `icon` (e.g. "Edit", "Delete").
fn row_icon(h: &mut Harness<'_, State>, row: &str, icon: &str) {
    hover_row(h, row);
    h.get_by_label(icon).click();
    h.run();
}

/// Adds a sub-task under the top-level task named `parent`.
fn add_subtask(h: &mut Harness<'_, State>, parent: &str, title: &str) {
    row_icon(h, parent, "Add sub-task");
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

    row_icon(&mut h, "Checkout", "Delete");
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

#[test]
fn each_row_shows_todays_time_and_a_parent_includes_its_subtasks() {
    let mut h = checkout_with_two_subtasks();
    let tasks = h.state().store.list_live().unwrap();
    let id = |title: &str| tasks.iter().find(|t| t.title == title).unwrap().meta.id;
    let store = &h.state().store;
    store
        .start(wt_core::TimerTarget::Task(id("Validation")))
        .unwrap();
    h.state().clock.advance(jiff::SignedDuration::from_mins(25));
    store
        .stop_target(wt_core::TimerTarget::Task(id("Validation")))
        .unwrap();
    store.start(wt_core::TimerTarget::Task(id("API"))).unwrap();
    h.state().clock.advance(jiff::SignedDuration::from_mins(40));
    store
        .stop_target(wt_core::TimerTarget::Task(id("API")))
        .unwrap();
    h.run();

    h.get_by_label("40m");
    h.get_by_label("1h 05m");
}

#[test]
fn a_sub_task_can_have_sub_tasks_but_the_third_level_cannot() {
    let mut h = harness();
    add_task(&mut h, "Checkout");
    add_subtask(&mut h, "Checkout", "API");

    add_subtask(&mut h, "API", "Auth header");
    h.get_by_label("Auth header");

    hover_row(&mut h, "Auth header");
    assert!(h.query_by_label("Add sub-task").is_none());
}

#[test]
fn a_done_to_do_sinks_below_the_open_ones_in_its_group() {
    let mut h = harness();
    add_task(&mut h, "Checkout");
    add_subtask(&mut h, "Checkout", "Validation");
    add_subtask(&mut h, "Checkout", "API");
    let top = |h: &Harness<'_, State>, title: &str| h.get_by_label(title).rect().top();
    assert!(top(&h, "Validation") < top(&h, "API"));

    h.get_by_label("Validation").click();
    h.run();

    assert!(top(&h, "API") < top(&h, "Validation"));
    // Only the display moves; the stored order is still creation order.
    let titles: Vec<_> = h
        .state()
        .store
        .list_live()
        .unwrap()
        .into_iter()
        .map(|t| t.title)
        .collect();
    assert_eq!(titles, ["Checkout", "Validation", "API"]);
}

#[test]
fn the_category_tag_shows_on_the_project_heading_only() {
    let h = checkout_with_two_subtasks();

    // Sub-tasks share the heading's category, so only the heading tags it.
    assert_eq!(h.query_all_by_label("Office").count(), 1);
}

#[test]
fn the_inline_edit_card_saves_a_new_title() {
    let mut h = checkout_with_two_subtasks();

    row_icon(&mut h, "Validation", "Edit");
    h.get_by_label("Task title")
        .set_text_value_accesskit("Form validation");
    h.run();
    h.get_by_label("Save").click();
    h.run();

    h.get_by_label("Form validation");
    assert!(h.query_by_label("Task title").is_none());
    let tasks = h.state().store.list_live().unwrap();
    assert!(tasks.iter().any(|t| t.title == "Form validation"));
}

#[test]
fn the_inline_edit_card_saves_a_new_category_for_a_project() {
    let mut h = checkout_with_two_subtasks();

    row_icon(&mut h, "Checkout", "Edit");
    // The card's picker comes after the New To-Do line's.
    h.query_all_by_label("Category").last().unwrap().click();
    h.run();
    h.get_by_label("Learning").click();
    h.run();
    h.get_by_label("Save").click();
    h.run();

    let store = &h.state().store;
    let learning = store
        .list()
        .unwrap()
        .into_iter()
        .find(|c| c.name == "Learning")
        .unwrap();
    let tasks = store.list_live().unwrap();
    assert!(tasks.iter().all(|t| t.category_id == learning.meta.id));
    h.get_by_label("Learning");
}

/// The stored break interval of the task named `title`.
fn break_interval(h: &Harness<'_, State>, title: &str) -> Option<u32> {
    let tasks = h.state().store.list_live().unwrap();
    tasks
        .into_iter()
        .find(|t| t.title == title)
        .unwrap()
        .break_interval_min
}

/// Opens the edit card of `title` and its break drop-down.
fn open_break_picker(h: &mut Harness<'_, State>, title: &str) {
    row_icon(h, title, "Edit");
    h.get_by_label("Break interval").click();
    h.run();
}

#[test]
fn choosing_a_break_preset_saves_the_tasks_own_interval() {
    let mut h = checkout_with_two_subtasks();

    open_break_picker(&mut h, "Checkout");
    // The default names the Settings value.
    h.get_by_label("Default break (25m)");
    h.get_by_label("Break every 45m").click();
    h.run();
    h.get_by_label("Save").click();
    h.run();

    assert_eq!(break_interval(&h, "Checkout"), Some(45));
}

#[test]
fn choosing_the_default_break_clears_the_tasks_own_interval() {
    let mut h = checkout_with_two_subtasks();
    let id = h.state().store.list_live().unwrap()[0].meta.id;
    h.state().store.set_break_interval(id, Some(50)).unwrap();
    h.run();

    open_break_picker(&mut h, "Checkout");
    h.get_by_label("Default break (25m)").click();
    h.run();
    h.get_by_label("Save").click();
    h.run();

    assert_eq!(break_interval(&h, "Checkout"), None);
}

#[test]
fn a_custom_break_interval_is_offered_and_kept() {
    let mut h = checkout_with_two_subtasks();
    let id = h.state().store.list_live().unwrap()[0].meta.id;
    h.state().store.set_break_interval(id, Some(37)).unwrap();
    h.run();

    open_break_picker(&mut h, "Checkout");
    // Not a preset, yet listed, so choosing another value is never forced.
    h.get_by_label("Break every 37m");
    // Close the menu without choosing, then save: the value survives.
    h.get_by_label("Break interval").click();
    h.run();
    h.get_by_label("Save").click();
    h.run();

    assert_eq!(break_interval(&h, "Checkout"), Some(37));
}

#[test]
fn a_sub_task_has_no_break_picker() {
    let mut h = checkout_with_two_subtasks();

    row_icon(&mut h, "Validation", "Edit");

    assert!(h.query_by_label("Break interval").is_none());
}
