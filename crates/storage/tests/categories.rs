#![allow(clippy::unwrap_used)] // test code may unwrap (SPEC: Code Style)

use std::sync::Arc;

use jiff::{SignedDuration, Timestamp};
use wt_core::{CategoryKind, CategoryRepo, FixedClock, Id, NewCategory, RepoError, Rgb};
use wt_storage::SqliteStore;

fn at(ts: &str) -> Timestamp {
    ts.parse().unwrap()
}

fn store_at(ts: &str) -> (SqliteStore, Arc<FixedClock>) {
    let clock = Arc::new(FixedClock::utc(at(ts)));
    (SqliteStore::open_in_memory(clock.clone()).unwrap(), clock)
}

fn names(store: &SqliteStore) -> Vec<String> {
    store.list().unwrap().into_iter().map(|c| c.name).collect()
}

#[test]
fn fresh_database_is_seeded_once_with_the_default_categories() {
    let (store, _) = store_at("2026-10-21T09:00:00Z");

    assert!(store.seed_defaults().unwrap());
    assert!(!store.seed_defaults().unwrap());

    assert_eq!(
        names(&store),
        [
            "Office",
            "Meeting",
            "Learning",
            "Family",
            "Personal",
            "Social media",
            "Break"
        ]
    );
}

#[test]
fn deleting_every_category_does_not_bring_the_defaults_back() {
    let (store, _) = store_at("2026-10-21T09:00:00Z");
    store.seed_defaults().unwrap();
    for c in store.list().unwrap() {
        store.delete(c.meta.id).unwrap();
    }

    assert!(!store.seed_defaults().unwrap());
    assert!(store.list().unwrap().is_empty());
}

#[test]
fn created_category_is_listed_with_sync_metadata() {
    let (store, _) = store_at("2026-10-21T09:00:00Z");

    let made = store
        .create(NewCategory::new(
            "Rust study",
            CategoryKind::Learning,
            Rgb(1, 2, 3),
        ))
        .unwrap();

    let listed = store.list().unwrap();
    assert_eq!(listed, std::slice::from_ref(&made));
    assert_eq!(made.kind, CategoryKind::Learning);
    assert_eq!(made.color, Rgb(1, 2, 3));
    assert_eq!(made.meta.created_at, at("2026-10-21T09:00:00Z"));
    assert_eq!(made.meta.updated_at, at("2026-10-21T09:00:00Z"));
    assert_eq!(made.meta.deleted_at, None);
}

#[test]
fn updating_bumps_updated_at_but_keeps_created_at() {
    let (store, clock) = store_at("2026-10-21T09:00:00Z");
    let made = store
        .create(NewCategory::new(
            "Gym",
            CategoryKind::Personal,
            Rgb(0, 0, 0),
        ))
        .unwrap();
    clock.advance(SignedDuration::from_hours(2));

    let updated = store
        .update(made.meta.id, "Gym & run", Rgb(255, 0, 0))
        .unwrap();

    assert_eq!(updated.name, "Gym & run");
    assert_eq!(updated.color, Rgb(255, 0, 0));
    assert_eq!(updated.meta.created_at, at("2026-10-21T09:00:00Z"));
    assert_eq!(updated.meta.updated_at, at("2026-10-21T11:00:00Z"));
    assert_eq!(store.list().unwrap(), [updated]);
}

#[test]
fn deleted_category_disappears_from_the_list() {
    let (store, _) = store_at("2026-10-21T09:00:00Z");
    let keep = store
        .create(NewCategory::new("Keep", CategoryKind::Custom, Rgb(0, 0, 0)))
        .unwrap();
    let gone = store
        .create(NewCategory::new("Gone", CategoryKind::Custom, Rgb(0, 0, 0)))
        .unwrap();

    store.delete(gone.meta.id).unwrap();

    assert_eq!(store.list().unwrap(), [keep]);
}

#[test]
fn changing_a_missing_or_deleted_category_is_not_found() {
    let (store, _) = store_at("2026-10-21T09:00:00Z");
    let gone = store
        .create(NewCategory::new("Gone", CategoryKind::Custom, Rgb(0, 0, 0)))
        .unwrap();
    store.delete(gone.meta.id).unwrap();

    let missing = Id::new();
    assert!(matches!(store.delete(missing), Err(RepoError::NotFound(id)) if id == missing));
    assert!(matches!(
        store.update(gone.meta.id, "Back?", Rgb(0, 0, 0)),
        Err(RepoError::NotFound(_))
    ));
}

#[test]
fn categories_made_within_the_same_second_stay_in_creation_order() {
    let (store, clock) = store_at("2026-10-21T09:00:00Z");
    let first = store
        .create(NewCategory::new(
            "First",
            CategoryKind::Custom,
            Rgb(0, 0, 0),
        ))
        .unwrap();
    clock.advance(SignedDuration::from_millis(500));
    let second = store
        .create(NewCategory::new(
            "Second",
            CategoryKind::Custom,
            Rgb(0, 0, 0),
        ))
        .unwrap();

    assert_eq!(store.list().unwrap(), [first, second]);
}
