#![allow(clippy::unwrap_used)] // test code may unwrap (SPEC: Code Style)

use std::sync::Arc;

use jiff::Timestamp;
use jiff::civil::Weekday;
use wt_core::{BlockDraft, CategoryRepo, FixedClock, Id, Period, PlanRepo, RepoError, Weekdays};
use wt_storage::SqliteStore;

fn store() -> (SqliteStore, Id) {
    let clock = Arc::new(FixedClock::utc(
        "2026-10-22T03:00:00Z".parse::<Timestamp>().unwrap(),
    ));
    let store = SqliteStore::open_in_memory(clock).unwrap();
    store.seed_defaults().unwrap();
    let learning = store
        .list()
        .unwrap()
        .into_iter()
        .find(|c| c.name == "Learning")
        .unwrap()
        .meta
        .id;
    (store, learning)
}

fn evenings(category: Id) -> BlockDraft {
    BlockDraft {
        category_id: category,
        weekdays: Weekdays::from_days(&[Weekday::Sunday, Weekday::Monday]),
        start_min: 21 * 60,
        end_min: 22 * 60,
        label: " Rust study ".to_owned(),
    }
}

#[test]
fn setting_a_target_again_updates_it_instead_of_adding_another() {
    let (store, learning) = store();

    store.set_target(learning, Period::Week, 5).unwrap();
    store.set_target(learning, Period::Week, 7).unwrap();
    store.set_target(learning, Period::Month, 20).unwrap();

    let mut targets: Vec<_> = store
        .targets()
        .unwrap()
        .into_iter()
        .map(|t| (t.period, t.minutes))
        .collect();
    targets.sort_by_key(|(p, _)| p.as_str());
    assert_eq!(targets, [(Period::Month, 20 * 60), (Period::Week, 7 * 60)]);
}

#[test]
fn target_hours_must_be_within_the_periods_limits() {
    let (store, learning) = store();

    assert!(matches!(
        store.set_target(learning, Period::Week, 0),
        Err(RepoError::Invalid(_))
    ));
    assert!(matches!(
        store.set_target(learning, Period::Week, 201),
        Err(RepoError::Invalid(_))
    ));
    store.set_target(learning, Period::Month, 800).unwrap();
}

#[test]
fn a_deleted_target_is_gone() {
    let (store, learning) = store();
    let target = store.set_target(learning, Period::Week, 5).unwrap();

    store.delete_target(target.meta.id).unwrap();

    assert!(store.targets().unwrap().is_empty());
}

#[test]
fn a_block_is_saved_with_a_trimmed_label() {
    let (store, learning) = store();

    let block = store.add_block(&evenings(learning)).unwrap();

    assert_eq!(store.blocks().unwrap(), std::slice::from_ref(&block));
    assert_eq!(block.label, "Rust study");
    assert_eq!((block.start_min, block.end_min), (21 * 60, 22 * 60));
    assert!(block.weekdays.contains(Weekday::Monday));
    assert!(!block.weekdays.contains(Weekday::Friday));
}

#[test]
fn a_block_needs_a_day_and_must_end_after_it_starts_on_the_same_day() {
    let (store, learning) = store();
    let no_days = BlockDraft {
        weekdays: Weekdays::default(),
        ..evenings(learning)
    };
    let backwards = BlockDraft {
        start_min: 22 * 60,
        end_min: 21 * 60,
        ..evenings(learning)
    };
    let past_midnight = BlockDraft {
        end_min: 24 * 60 + 30,
        ..evenings(learning)
    };

    for draft in [no_days, backwards, past_midnight] {
        assert!(matches!(
            store.add_block(&draft),
            Err(RepoError::Invalid(_))
        ));
    }
    assert!(store.blocks().unwrap().is_empty());
}

#[test]
fn a_block_can_be_changed_and_deleted() {
    let (store, learning) = store();
    let block = store.add_block(&evenings(learning)).unwrap();
    let later = BlockDraft {
        start_min: 22 * 60,
        end_min: 23 * 60,
        ..evenings(learning)
    };

    let changed = store.update_block(block.meta.id, &later).unwrap();
    assert_eq!((changed.start_min, changed.end_min), (22 * 60, 23 * 60));

    store.delete_block(block.meta.id).unwrap();
    assert!(store.blocks().unwrap().is_empty());
}
