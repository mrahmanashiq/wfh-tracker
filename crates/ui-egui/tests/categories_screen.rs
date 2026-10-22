#![allow(clippy::unwrap_used)] // test code may unwrap (SPEC: Code Style)

use std::sync::Arc;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use jiff::Timestamp;
use wt_core::{CategoryRepo, FixedClock, Placement, TaskRepo};
use wt_storage::SqliteStore;
use wt_ui_egui::CategoriesScreen;

struct State {
    store: SqliteStore,
    screen: CategoriesScreen,
}

fn harness() -> Harness<'static, State> {
    let clock = Arc::new(FixedClock::utc(
        "2026-10-22T04:00:00Z".parse::<Timestamp>().unwrap(),
    ));
    let store = SqliteStore::open_in_memory(clock).unwrap();
    store.seed_defaults().unwrap();
    Harness::new_ui_state(
        |ui, s: &mut State| s.screen.ui(ui, &s.store),
        State {
            store,
            screen: CategoriesScreen::default(),
        },
    )
}

#[test]
fn a_new_category_is_added() {
    let mut h = harness();

    h.get_by_label("Category name")
        .set_text_value_accesskit("Gym");
    h.run();
    h.get_by_label("Add category").click();
    h.run();

    h.get_by_label("Gym");
    let names: Vec<_> = h
        .state()
        .store
        .list()
        .unwrap()
        .into_iter()
        .map(|c| c.name)
        .collect();
    assert!(names.contains(&"Gym".to_owned()), "{names:?}");
}

#[test]
fn a_category_in_use_is_not_deleted_and_the_tasks_are_named() {
    let mut h = harness();
    let office = h.state().store.list().unwrap()[0].clone();
    h.state()
        .store
        .add(
            "Checkout",
            Placement::TopLevel {
                category_id: office.meta.id,
            },
        )
        .unwrap();
    h.run();

    // Office is the first category, so its Delete button comes first.
    h.get_all_by_label("Delete").next().unwrap().click();
    h.run();
    h.get_by_label("Confirm delete").click();
    h.run();

    h.get_by_label("still used by: Checkout");
    assert_eq!(h.state().store.list().unwrap()[0].name, "Office");
}
