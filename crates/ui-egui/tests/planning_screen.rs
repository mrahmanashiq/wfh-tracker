#![allow(clippy::unwrap_used)] // test code may unwrap (SPEC: Code Style)

use std::sync::Arc;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use jiff::Timestamp;
use wt_core::{BlockRepo, FixedClock, Settings, TargetRepo};
use wt_storage::SqliteStore;
use wt_ui_egui::PlanningScreen;

struct State {
    store: SqliteStore,
    clock: Arc<FixedClock>,
    screen: PlanningScreen,
}

fn harness() -> Harness<'static, State> {
    let clock = Arc::new(FixedClock::utc(
        "2026-10-22T03:00:00Z".parse::<Timestamp>().unwrap(),
    ));
    let store = SqliteStore::open_in_memory(clock.clone()).unwrap();
    wt_core::CategoryRepo::seed_defaults(&store).unwrap();
    Harness::new_ui_state(
        |ui, s: &mut State| {
            s.screen
                .ui(ui, &s.store, &Settings::default(), s.clock.as_ref());
        },
        State {
            store,
            clock,
            screen: PlanningScreen::default(),
        },
    )
}

#[test]
fn an_added_target_shows_its_progress_for_this_week() {
    let mut h = harness();

    h.get_by_label("Target hours").set_value_accesskit(5.0);
    h.run();
    h.get_by_label("Add target").click();
    h.run();

    h.get_by_label("Learning · this week");
    h.get_by_label("0h 00m / 5h");
    assert_eq!(h.state().store.targets().unwrap()[0].minutes, 5 * 60);
}

#[test]
fn an_added_block_is_listed_with_its_days_and_times() {
    let mut h = harness();

    h.get_by_label("Block label")
        .set_text_value_accesskit("Rust study");
    h.run();
    h.get_by_label("Add block").click();
    h.run();

    h.get_by_label("Rust study");
    h.get_by_label("Sun Mon Tue Wed Thu · 21:00–22:00");
    assert_eq!(h.state().store.blocks().unwrap().len(), 1);
}

#[test]
fn targets_and_blocks_can_be_deleted() {
    let mut h = harness();
    h.get_by_label("Target hours").set_value_accesskit(5.0);
    h.run();
    h.get_by_label("Add target").click();
    h.run();
    h.get_by_label("Add block").click();
    h.run();

    for _ in 0..2 {
        h.get_all_by_label("Delete").next().unwrap().click();
        h.run();
        h.get_by_label("Confirm delete").click();
        h.run();
    }

    assert!(h.state().store.targets().unwrap().is_empty());
    assert!(h.state().store.blocks().unwrap().is_empty());
}

#[test]
fn editing_a_target_changes_it_instead_of_adding_another() {
    let mut h = harness();
    h.get_by_label("Target hours").set_value_accesskit(5.0);
    h.run();
    h.get_by_label("Add target").click();
    h.run();

    h.get_all_by_label("Edit").next().unwrap().click();
    h.run();
    h.get_by_label("Target hours").set_value_accesskit(7.0);
    h.run();
    h.get_by_label("Save target").click();
    h.run();

    let targets = h.state().store.targets().unwrap();
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].minutes, 7 * 60);
}
