#![allow(clippy::unwrap_used)] // test code may unwrap (SPEC: Code Style)

use std::sync::Arc;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use jiff::{SignedDuration, Timestamp};
use wt_core::{
    CategoryRepo, DistractionEventRepo, DistractionRuleRepo, FixedClock, Period, Placement,
    SessionRepo, Settings, TargetRepo, TaskRepo, TimerTarget, WatchContext,
};
use wt_storage::SqliteStore;
use wt_ui_egui::DashboardScreen;

struct State {
    store: SqliteStore,
    clock: Arc<FixedClock>,
    screen: DashboardScreen,
}

/// Sunday 18 Oct 2026: 7h 30m on "Checkout". Thursday 22 Oct: "Checkout"
/// 09:00-10:30 with 15 minutes of Facebook in it, then 30 minutes of
/// learning against a 5h weekly target. Now is Thursday 11:00 (UTC).
fn harness() -> Harness<'static, State> {
    let clock = Arc::new(FixedClock::utc(
        "2026-10-18T02:00:00Z".parse::<Timestamp>().unwrap(),
    ));
    let store = SqliteStore::open_in_memory(clock.clone()).unwrap();
    store.seed_defaults().unwrap();
    store.seed_default_rules().unwrap();
    let cats = store.list().unwrap();
    let id_of = |name: &str| cats.iter().find(|c| c.name == name).unwrap().meta.id;
    let checkout = store
        .add(
            "Checkout",
            Placement::TopLevel {
                category_id: id_of("Office"),
            },
        )
        .unwrap();
    store
        .set_target(id_of("Learning"), Period::Week, 5)
        .unwrap();

    store.start(TimerTarget::Task(checkout.meta.id)).unwrap();
    clock.advance(SignedDuration::from_mins(7 * 60 + 30));
    store.stop().unwrap();

    // Sunday 09:30 to Thursday 09:00.
    clock.advance(SignedDuration::from_mins(4 * 24 * 60 - 30));
    store.start(TimerTarget::Task(checkout.meta.id)).unwrap();
    clock.advance(SignedDuration::from_mins(90));
    store
        .start(TimerTarget::Category(id_of("Learning")))
        .unwrap();
    clock.advance(SignedDuration::from_mins(30));
    store.stop().unwrap();

    let facebook = store.rules().unwrap()[0].meta.id;
    store
        .insert_distraction(
            facebook,
            WatchContext::Office,
            "2026-10-22T10:00:00Z".parse().unwrap(),
        )
        .unwrap();
    store
        .end_distractions("2026-10-22T10:15:00Z".parse().unwrap())
        .unwrap();

    let mut h = Harness::builder()
        .with_size(egui::vec2(1000.0, 900.0))
        .build_ui_state(
            |ui, s: &mut State| {
                s.screen
                    .ui(ui, &s.store, &Settings::default(), s.clock.as_ref());
            },
            State {
                store,
                clock,
                screen: DashboardScreen,
            },
        );
    h.run();
    h
}

#[test]
fn the_office_card_shows_todays_office_time_against_the_target() {
    let h = harness();

    h.get_by_label("Office 1h 30m / 8h");
    h.get_by_label("Learning 0h 30m");
    h.get_by_label("Distractions 0h 15m");
}

#[test]
fn each_timeline_segment_is_named_after_its_task_and_times() {
    let h = harness();

    h.get_by_label("Checkout 09:00-10:30");
    h.get_by_label("Learning 10:30-11:00");
}

#[test]
fn each_weekday_bar_is_named_after_its_day_and_office_time() {
    let h = harness();

    h.get_by_label("Sun 7h 30m");
    h.get_by_label("Thu 1h 30m");
    h.get_by_label("Fri 0h 00m - day off");
}

#[test]
fn a_category_with_a_weekly_target_shows_its_progress() {
    let h = harness();

    h.get_by_label("Learning 0h 30m / 5h");
    h.get_by_label("Office 9h 00m");
}

#[test]
fn the_weeks_distractions_show_the_total_and_each_rule() {
    let h = harness();

    h.get_by_label("Total 0h 15m");
    h.get_by_label("Facebook 15m");
}

#[test]
fn on_a_day_off_the_office_card_says_so() {
    let mut h = harness();

    h.state().clock.advance(SignedDuration::from_hours(24));
    h.run();

    h.get_by_label("Office 0h 00m - day off");
}
