#![allow(clippy::unwrap_used)] // test code may unwrap (SPEC: Code Style)

use std::sync::Arc;

use jiff::Timestamp;
use jiff::civil::Weekday::*;
use wt_core::{Appearance, FixedClock, Settings, SettingsRepo};
use wt_storage::SqliteStore;

fn store() -> SqliteStore {
    let clock = Arc::new(FixedClock::utc(Timestamp::UNIX_EPOCH));
    SqliteStore::open_in_memory(clock).unwrap()
}

#[test]
fn fresh_database_loads_the_spec_defaults() {
    let settings = store().load().unwrap();

    assert_eq!(
        settings,
        Settings {
            office_target_min: 8 * 60,
            office_ceiling_min: 9 * 60,
            break_interval_min: 25,
            break_length_min: 5,
            distraction_grace_secs: 60,
            nudge_cooldown_secs: 120,
            week_start: Sunday,
            workdays: vec![Sunday, Monday, Tuesday, Wednesday, Thursday],
            autostart: true,
            appearance: Appearance::Light,
        }
    );
}

#[test]
fn saved_settings_load_back() {
    let store = store();
    let mine = Settings {
        office_target_min: 7 * 60 + 30,
        office_ceiling_min: 8 * 60 + 30,
        break_interval_min: 50,
        break_length_min: 10,
        distraction_grace_secs: 30,
        nudge_cooldown_secs: 300,
        week_start: Monday,
        workdays: vec![Monday, Tuesday, Wednesday, Thursday, Friday],
        autostart: false,
        appearance: Appearance::Dark,
    };

    store.save(&mine).unwrap();

    assert_eq!(store.load().unwrap(), mine);
}

#[test]
fn ceiling_below_target_is_rejected_and_nothing_is_stored() {
    let store = store();
    let bad = Settings {
        office_target_min: 9 * 60,
        office_ceiling_min: 8 * 60,
        ..Settings::default()
    };

    assert!(store.save(&bad).is_err());
    assert_eq!(store.load().unwrap(), Settings::default());
}

#[test]
fn settings_survive_reopening_the_database_file() {
    let path = std::env::temp_dir().join(format!("wt-settings-{}.db", wt_core::Id::new()));
    let clock = Arc::new(FixedClock::utc(Timestamp::UNIX_EPOCH));
    let mine = Settings {
        break_interval_min: 45,
        ..Settings::default()
    };

    SqliteStore::open(&path, clock.clone())
        .unwrap()
        .save(&mine)
        .unwrap();
    let reopened = SqliteStore::open(&path, clock).unwrap().load().unwrap();

    assert_eq!(reopened.break_interval_min, 45);
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
}
