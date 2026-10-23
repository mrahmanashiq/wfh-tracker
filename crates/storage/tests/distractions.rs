#![allow(clippy::unwrap_used)] // test code may unwrap (SPEC: Code Style)

use std::sync::Arc;

use jiff::{SignedDuration, Timestamp};
use wt_core::{
    DistractionRepo, FixedClock, MatchOn, NudgeCommand, RepoError, RuleDraft, WatchContext,
};
use wt_storage::SqliteStore;

fn ts(s: &str) -> Timestamp {
    s.parse().unwrap()
}

fn store() -> (SqliteStore, Arc<FixedClock>) {
    let clock = Arc::new(FixedClock::utc(ts("2026-10-22T21:00:00Z")));
    (SqliteStore::open_in_memory(clock.clone()).unwrap(), clock)
}

fn patterns(store: &SqliteStore) -> Vec<(String, MatchOn, bool)> {
    store
        .rules()
        .unwrap()
        .into_iter()
        .map(|r| (r.pattern, r.match_on, r.enabled))
        .collect()
}

#[test]
fn the_default_rules_are_seeded_once_in_order() {
    let (store, _) = store();

    assert!(store.seed_default_rules().unwrap());
    assert!(!store.seed_default_rules().unwrap());

    assert_eq!(
        patterns(&store),
        [
            ("Facebook".to_owned(), MatchOn::Title, true),
            ("Reels".to_owned(), MatchOn::Title, true),
            ("Shorts".to_owned(), MatchOn::Title, true),
            ("Instagram".to_owned(), MatchOn::Title, true),
        ]
    );
}

#[test]
fn deleting_every_rule_does_not_bring_the_defaults_back() {
    let (store, _) = store();
    store.seed_default_rules().unwrap();
    for rule in store.rules().unwrap() {
        store.delete_rule(rule.meta.id).unwrap();
    }

    assert!(!store.seed_default_rules().unwrap());
    assert_eq!(patterns(&store), []);
}

#[test]
fn a_rule_can_be_added_disabled_and_deleted() {
    let (store, _) = store();

    let rule = store
        .add_rule(&RuleDraft::new(" discord ", MatchOn::Process))
        .unwrap();
    assert_eq!(rule.pattern, "discord");
    store.set_rule_enabled(rule.meta.id, false).unwrap();
    assert_eq!(
        patterns(&store),
        [("discord".to_owned(), MatchOn::Process, false)]
    );

    store.delete_rule(rule.meta.id).unwrap();
    assert_eq!(patterns(&store), []);
    assert!(matches!(
        store.delete_rule(rule.meta.id),
        Err(RepoError::NotFound(_))
    ));
    assert!(matches!(
        store.set_rule_enabled(rule.meta.id, true),
        Err(RepoError::NotFound(_))
    ));
}

#[test]
fn a_rule_needs_1_to_60_characters() {
    let (store, _) = store();

    for bad in ["  ", &"y".repeat(61)] {
        assert!(matches!(
            store.add_rule(&RuleDraft::new(bad, MatchOn::Title)),
            Err(RepoError::Invalid(_))
        ));
    }
    assert_eq!(patterns(&store), []);
}

#[test]
fn watcher_commands_start_and_end_one_event() {
    let (store, clock) = store();
    store.seed_default_rules().unwrap();
    let facebook = store.rules().unwrap()[0].meta.id;

    store
        .record(&NudgeCommand::StartEvent {
            rule: facebook,
            context: WatchContext::Learning,
            started_at: ts("2026-10-22T21:00:00Z"),
        })
        .unwrap();
    store
        .record(&NudgeCommand::Nudge {
            rule: facebook,
            context: WatchContext::Learning,
        })
        .unwrap();
    clock.advance(SignedDuration::from_mins(3));
    let open = store
        .distractions_since(ts("2026-10-22T00:00:00Z"))
        .unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].ended_at, None);

    store
        .record(&NudgeCommand::EndEvent {
            at: ts("2026-10-22T21:03:00Z"),
        })
        .unwrap();

    let events = store
        .distractions_since(ts("2026-10-22T00:00:00Z"))
        .unwrap();
    let e = &events[0];
    assert_eq!(
        (e.rule_id, e.context, e.started_at, e.ended_at),
        (
            facebook,
            WatchContext::Learning,
            ts("2026-10-22T21:00:00Z"),
            Some(ts("2026-10-22T21:03:00Z"))
        )
    );
    assert_eq!(e.meta.updated_at, ts("2026-10-22T21:03:00Z"));
}

#[test]
fn starting_an_event_ends_one_left_open() {
    let (store, _) = store();
    store.seed_default_rules().unwrap();
    let rules = store.rules().unwrap();
    let (facebook, reels) = (rules[0].meta.id, rules[1].meta.id);

    store
        .insert_distraction(facebook, WatchContext::Office, ts("2026-10-22T09:00:00Z"))
        .unwrap();
    store
        .record(&NudgeCommand::StartEvent {
            rule: reels,
            context: WatchContext::Office,
            started_at: ts("2026-10-22T09:05:00Z"),
        })
        .unwrap();

    let ends: Vec<_> = store
        .distractions_since(ts("2026-10-22T00:00:00Z"))
        .unwrap()
        .into_iter()
        .map(|e| e.ended_at)
        .collect();
    assert_eq!(ends, [Some(ts("2026-10-22T09:05:00Z")), None]);
}

#[test]
fn an_open_event_is_never_ended_before_it_started() {
    let (store, _) = store();
    store.seed_default_rules().unwrap();
    let facebook = store.rules().unwrap()[0].meta.id;
    store
        .insert_distraction(facebook, WatchContext::Office, ts("2026-10-22T09:00:10Z"))
        .unwrap();

    // E.g. the last heartbeat was a few seconds before the event began.
    store.end_distractions(ts("2026-10-22T09:00:00Z")).unwrap();

    let e = &store
        .distractions_since(ts("2026-10-22T00:00:00Z"))
        .unwrap()[0];
    assert_eq!(e.ended_at, Some(ts("2026-10-22T09:00:10Z")));
}

#[test]
fn events_ended_before_since_are_left_out() {
    let (store, _) = store();
    store.seed_default_rules().unwrap();
    let facebook = store.rules().unwrap()[0].meta.id;
    store
        .insert_distraction(facebook, WatchContext::Office, ts("2026-10-21T09:00:00Z"))
        .unwrap();
    store.end_distractions(ts("2026-10-21T09:10:00Z")).unwrap();
    store
        .insert_distraction(facebook, WatchContext::Office, ts("2026-10-22T09:00:00Z"))
        .unwrap();

    let starts: Vec<_> = store
        .distractions_since(ts("2026-10-22T00:00:00Z"))
        .unwrap()
        .into_iter()
        .map(|e| e.started_at)
        .collect();
    assert_eq!(starts, [ts("2026-10-22T09:00:00Z")]);
}

#[test]
fn a_deleted_rules_events_are_kept() {
    let (store, _) = store();
    store.seed_default_rules().unwrap();
    let facebook = store.rules().unwrap()[0].meta.id;
    store
        .insert_distraction(facebook, WatchContext::Office, ts("2026-10-22T09:00:00Z"))
        .unwrap();
    store.end_distractions(ts("2026-10-22T09:10:00Z")).unwrap();

    store.delete_rule(facebook).unwrap();

    let events = store
        .distractions_since(ts("2026-10-22T00:00:00Z"))
        .unwrap();
    assert_eq!(events[0].rule_id, facebook);
}
