#![allow(clippy::unwrap_used)] // test code may unwrap (SPEC: Code Style)

use std::sync::Arc;

use jiff::{SignedDuration, Timestamp};
use wt_core::{
    CategoryRepo, Clock, FixedClock, Placement, SessionRepo, SettingsRepo, TaskRepo, TimerTarget,
};
use wt_storage::SqliteStore;
use wt_ui_egui::CachedStore;

fn store() -> (CachedStore<SqliteStore>, Arc<FixedClock>) {
    let clock = Arc::new(FixedClock::utc(
        "2026-10-22T09:00:00Z".parse::<Timestamp>().unwrap(),
    ));
    let store = SqliteStore::open_in_memory(clock.clone()).unwrap();
    store.seed_defaults().unwrap();
    (CachedStore::new(store), clock)
}

fn office(store: &CachedStore<SqliteStore>) -> wt_core::Id {
    store.list().unwrap()[0].meta.id
}

#[test]
fn reads_are_kept_until_a_write_through_the_cache() {
    let (cached, _) = store();
    let category_id = office(&cached);
    assert!(cached.list_live().unwrap().is_empty());

    // A write that bypasses the cache is not seen: the read was cached.
    cached
        .get_ref()
        .add("Behind its back", Placement::TopLevel { category_id })
        .unwrap();
    assert!(cached.list_live().unwrap().is_empty());

    // A write through it drops the cache, so both tasks show.
    let before = cached.revision();
    cached
        .add("Checkout", Placement::TopLevel { category_id })
        .unwrap();
    assert!(cached.revision() > before);
    assert_eq!(cached.list_live().unwrap().len(), 2);
}

#[test]
fn invalidate_reloads_everything() {
    let (cached, _) = store();
    let mut settings = cached.load().unwrap();
    settings.office_target_min = 7 * 60;
    cached.get_ref().save(&settings).unwrap();
    assert_ne!(cached.load().unwrap().office_target_min, 7 * 60);
    cached.invalidate();
    assert_eq!(cached.load().unwrap().office_target_min, 7 * 60);
}

#[test]
fn later_windows_are_filtered_from_the_cached_one() {
    let (cached, clock) = store();
    let category_id = office(&cached);
    let task = cached
        .add("Checkout", Placement::TopLevel { category_id })
        .unwrap();
    let start: Timestamp = "2026-10-22T09:00:00Z".parse().unwrap();
    for _ in 0..3 {
        cached.start(TimerTarget::Task(task.meta.id)).unwrap();
        clock.advance(SignedDuration::from_mins(60));
        cached.stop().unwrap();
        clock.advance(SignedDuration::from_mins(30));
    }
    cached.start(TimerTarget::Task(task.meta.id)).unwrap();

    // Load a wide window first, then ask for narrower ones.
    let wide = cached.sessions_since(start).unwrap();
    assert_eq!(wide.len(), 4);
    for minutes in [0, 59, 60, 61, 150, 200, 240, 300] {
        let since = start + SignedDuration::from_mins(minutes);
        assert_eq!(
            cached.sessions_since(since).unwrap(),
            cached.get_ref().sessions_since(since).unwrap(),
            "since +{minutes} min"
        );
    }
    // An earlier window than the cached one goes back to the store.
    let earlier = start - SignedDuration::from_hours(1);
    assert_eq!(cached.sessions_since(earlier).unwrap().len(), 4);
}

#[test]
fn a_running_timer_is_cached_but_its_time_keeps_counting() {
    let (cached, clock) = store();
    let category_id = office(&cached);
    let task = cached
        .add("Checkout", Placement::TopLevel { category_id })
        .unwrap();
    cached.start(TimerTarget::Task(task.meta.id)).unwrap();
    let running = cached.running().unwrap().unwrap();
    clock.advance(SignedDuration::from_mins(5));
    // Same cached row; elapsed time comes from the clock, not the cache.
    assert_eq!(cached.running().unwrap().unwrap(), running);
    assert_eq!(
        clock.now().duration_since(running.started_at),
        SignedDuration::from_mins(5)
    );
}
