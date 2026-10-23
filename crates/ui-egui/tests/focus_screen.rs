#![allow(clippy::unwrap_used)] // test code may unwrap (SPEC: Code Style)

use std::sync::Arc;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use jiff::Timestamp;
use wt_core::{DistractionRuleRepo, FixedClock, MatchOn};
use wt_storage::SqliteStore;
use wt_ui_egui::FocusScreen;

struct State {
    store: SqliteStore,
    screen: FocusScreen,
}

fn harness() -> Harness<'static, State> {
    let clock = Arc::new(FixedClock::utc(
        "2026-10-22T04:00:00Z".parse::<Timestamp>().unwrap(),
    ));
    let store = SqliteStore::open_in_memory(clock).unwrap();
    store.seed_default_rules().unwrap();
    Harness::new_ui_state(
        |ui, s: &mut State| s.screen.ui(ui, &s.store, None),
        State {
            store,
            screen: FocusScreen::default(),
        },
    )
}

fn rules(h: &Harness<'_, State>) -> Vec<(String, MatchOn, bool)> {
    h.state()
        .store
        .rules()
        .unwrap()
        .into_iter()
        .map(|r| (r.pattern, r.match_on, r.enabled))
        .collect()
}

#[test]
fn the_default_rules_are_listed_with_what_they_match() {
    let h = harness();

    for pattern in ["Facebook", "Reels", "Shorts", "Instagram"] {
        h.get_by_label(pattern);
    }
    assert_eq!(h.get_all_by_label("in the window title").count(), 4);
}

#[test]
fn a_rule_can_be_added_for_a_process_name() {
    let mut h = harness();

    h.get_by_label("Rule text")
        .set_text_value_accesskit("discord");
    h.run();
    h.get_by_label("Match on").click();
    h.run();
    h.get_by_label("Process name").click();
    h.run();
    h.get_by_label("Add rule").click();
    h.run();

    h.get_by_label("discord");
    h.get_by_label("in the process name");
    assert_eq!(
        rules(&h).last().unwrap(),
        &("discord".to_owned(), MatchOn::Process, true)
    );
}

#[test]
fn an_empty_rule_is_refused_with_a_message() {
    let mut h = harness();

    h.get_by_label("Add rule").click();
    h.run();

    h.get_by_label("a rule needs some text to match");
    assert_eq!(rules(&h).len(), 4);
}

#[test]
fn a_rule_can_be_turned_off() {
    let mut h = harness();

    h.get_by_label("Reels").click();
    h.run();

    assert_eq!(rules(&h)[1], ("Reels".to_owned(), MatchOn::Title, false));
}

#[test]
fn a_rule_is_deleted_after_confirming() {
    let mut h = harness();

    h.get_all_by_label("Delete").next().unwrap().click();
    h.run();
    h.get_by_label("Confirm delete").click();
    h.run();

    assert!(h.query_by_label("Facebook").is_none());
    assert_eq!(rules(&h).len(), 3);
}

#[test]
fn a_poll_error_is_shown() {
    let clock = Arc::new(FixedClock::utc(
        "2026-10-22T04:00:00Z".parse::<Timestamp>().unwrap(),
    ));
    let store = SqliteStore::open_in_memory(clock).unwrap();
    let h = Harness::new_ui_state(
        |ui, s: &mut State| s.screen.ui(ui, &s.store, Some("disk full")),
        State {
            store,
            screen: FocusScreen::default(),
        },
    );

    h.get_by_label("Could not check for distractions: disk full");
}
