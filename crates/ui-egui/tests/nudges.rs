#![allow(clippy::unwrap_used)] // test code may unwrap (SPEC: Code Style)

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::Arc;

use jiff::civil::Weekday;
use jiff::{SignedDuration, Timestamp};
use wt_core::{
    BlockDraft, BlockRepo, CategoryRepo, DistractionEventRepo, DistractionRuleRepo, FixedClock,
    ForegroundWatcher, ForegroundWindow, Id, Notifier, SessionRepo, Settings, TimerTarget,
    WatchContext, Weekdays,
};
use wt_storage::SqliteStore;
use wt_ui_egui::Nudges;

#[derive(Default)]
struct FakeNotifier(RefCell<Vec<(String, String)>>);

impl Notifier for FakeNotifier {
    fn notify(&self, title: &str, body: &str) {
        self.0
            .borrow_mut()
            .push((title.to_owned(), body.to_owned()));
    }
}

/// A foreground window the test can change.
#[derive(Default)]
struct FakeForeground(RefCell<Option<ForegroundWindow>>);

impl FakeForeground {
    fn show(&self, title: &str, process: &str) {
        *self.0.borrow_mut() = Some(ForegroundWindow {
            title: title.to_owned(),
            process: process.to_owned(),
        });
    }
}

impl ForegroundWatcher for FakeForeground {
    fn foreground(&self) -> Option<ForegroundWindow> {
        self.0.borrow().clone()
    }
}

const TITLE: &str = "Reels | Facebook - Google Chrome";

fn ts(s: &str) -> Timestamp {
    s.parse().unwrap()
}

/// Thursday 22 Oct 2026, 03:00 UTC, with the default categories and rules,
/// in a database file (so the test can look at what was written to disk).
struct App {
    dir: PathBuf,
    store: Option<SqliteStore>,
    clock: Arc<FixedClock>,
    nudges: Nudges,
    window: FakeForeground,
    notifier: FakeNotifier,
    settings: Settings,
}

impl App {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("wfh-tracker-nudges-{}", Id::new()));
        std::fs::create_dir_all(&dir).unwrap();
        let clock = Arc::new(FixedClock::utc(ts("2026-10-22T03:00:00Z")));
        let store = SqliteStore::open(&dir.join("test.db"), clock.clone()).unwrap();
        store.seed_defaults().unwrap();
        store.seed_default_rules().unwrap();
        Self {
            dir,
            store: Some(store),
            clock,
            nudges: Nudges::default(),
            window: FakeForeground::default(),
            notifier: FakeNotifier::default(),
            settings: Settings::default(),
        }
    }

    fn store(&self) -> &SqliteStore {
        self.store.as_ref().unwrap()
    }

    fn category(&self, name: &str) -> Id {
        self.store()
            .list()
            .unwrap()
            .into_iter()
            .find(|c| c.name == name)
            .unwrap()
            .meta
            .id
    }

    /// A Learning block on Thursdays, 03:00-04:00.
    fn learning_block(&self) {
        self.store()
            .add_block(&BlockDraft {
                category_id: self.category("Learning"),
                weekdays: Weekdays::from_days(&[Weekday::Thursday]),
                start_min: 3 * 60,
                end_min: 4 * 60,
                label: String::new(),
            })
            .unwrap();
    }

    /// Polls every 5 s, as the app does, for `secs` seconds.
    fn poll_for(&mut self, secs: i64) {
        for _ in 0..secs / 5 {
            self.poll();
            self.clock.advance(SignedDuration::from_secs(5));
        }
    }

    fn poll(&mut self) {
        let store = self.store.as_ref().unwrap();
        self.nudges
            .poll(
                store,
                &self.settings,
                self.clock.as_ref(),
                &self.window,
                &self.notifier,
            )
            .unwrap();
    }

    fn nudges(&self) -> Vec<(String, String)> {
        self.notifier.0.borrow().clone()
    }

    fn events(&self) -> Vec<(Id, WatchContext, Timestamp, Option<Timestamp>)> {
        self.store()
            .distractions_since(ts("2026-10-22T00:00:00Z"))
            .unwrap()
            .into_iter()
            .map(|e| (e.rule_id, e.context, e.started_at, e.ended_at))
            .collect()
    }

    fn rule(&self, pattern: &str) -> Id {
        self.store()
            .rules()
            .unwrap()
            .into_iter()
            .find(|r| r.pattern == pattern)
            .unwrap()
            .meta
            .id
    }

    /// Every byte SQLite wrote, after closing the database.
    fn database_bytes(&mut self) -> Vec<u8> {
        self.store = None;
        let mut bytes = Vec::new();
        for entry in std::fs::read_dir(&self.dir).unwrap() {
            bytes.extend(std::fs::read(entry.unwrap().path()).unwrap());
        }
        bytes
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.store = None;
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn contains(haystack: &[u8], needle: &str) -> bool {
    haystack
        .windows(needle.len())
        .any(|w| w == needle.as_bytes())
}

/// SPEC.md success criterion 5.
#[test]
fn reels_during_a_learning_block_nudges_once_and_records_the_event_without_the_title() {
    let mut app = App::new();
    app.learning_block();
    app.window.show(TITLE, "chrome.exe");

    app.poll_for(55);
    assert_eq!(app.nudges(), []);

    app.poll_for(125); // 03:00:55 to 03:03:00: past the grace, within the cooldown
    assert_eq!(
        app.nudges(),
        [(
            "Facebook during your Learning block".to_owned(),
            "Back to it?".to_owned()
        )]
    );

    app.window
        .show("main.rs - wfh-tracker - Visual Studio Code", "Code.exe");
    app.poll();

    assert_eq!(app.nudges().len(), 1);
    assert_eq!(
        app.events(),
        [(
            app.rule("Facebook"),
            WatchContext::Learning,
            ts("2026-10-22T03:00:00Z"),
            // The last poll that saw Facebook, not the one that saw VS Code.
            Some(ts("2026-10-22T03:02:55Z"))
        )]
    );
    let bytes = app.database_bytes();
    assert!(contains(&bytes, "Facebook"), "the rule itself is stored");
    for private in [TITLE, "Reels | Facebook", "Google Chrome", "chrome.exe"] {
        assert!(!contains(&bytes, private), "`{private}` was stored");
    }
}

#[test]
fn a_distraction_during_office_time_nudges_again_after_the_cooldown() {
    let mut app = App::new();
    let office = app.category("Office");
    app.store().start(TimerTarget::Category(office)).unwrap();
    app.window.show("Facebook", "msedge.exe");

    app.poll_for(185);

    let title = "Facebook during office time".to_owned();
    let titles: Vec<String> = app.nudges().into_iter().map(|(t, _)| t).collect();
    assert_eq!(titles, [title.clone(), title]);
    assert_eq!(app.events()[0].1, WatchContext::Office);
}

#[test]
fn a_learning_timer_outside_a_block_is_learning_time() {
    let mut app = App::new();
    let learning = app.category("Learning");
    app.store().start(TimerTarget::Category(learning)).unwrap();
    app.window.show("YouTube Shorts", "chrome.exe");

    app.poll_for(65);

    let titles: Vec<String> = app.nudges().into_iter().map(|(t, _)| t).collect();
    assert_eq!(titles, ["Shorts during learning time"]);
}

#[test]
fn nothing_is_watched_outside_protected_time() {
    let mut app = App::new();
    let family = app.category("Family");
    app.store().start(TimerTarget::Category(family)).unwrap();
    app.window.show(TITLE, "chrome.exe");

    app.poll_for(300);

    assert_eq!(app.nudges(), []);
    assert_eq!(app.events(), []);
}

#[test]
fn stopping_ends_the_open_event() {
    let mut app = App::new();
    app.learning_block();
    app.window.show(TITLE, "chrome.exe");
    app.poll_for(90); // polls 03:00:00 to 03:01:25

    let store = app.store.as_ref().unwrap();
    app.nudges.stop(store).unwrap();

    assert_eq!(app.events()[0].3, Some(ts("2026-10-22T03:01:25Z")));
}

#[test]
fn the_stored_duration_leaves_out_the_poll_that_no_longer_matched() {
    let mut app = App::new();
    app.learning_block();
    app.window.show("Facebook", "chrome.exe");
    app.poll_for(100); // matching polls 03:00:00 to 03:01:35

    app.window.show("Notes", "notepad.exe");
    app.poll(); // 03:01:40

    let (_, _, started, ended) = app.events()[0];
    assert_eq!(started, ts("2026-10-22T03:00:00Z"));
    assert_eq!(ended, Some(ts("2026-10-22T03:01:35Z")));
    assert_eq!(
        ended.unwrap().duration_since(started),
        SignedDuration::from_secs(95)
    );
}

#[test]
fn a_title_matching_two_rules_is_recorded_under_the_oldest() {
    let mut app = App::new();
    app.learning_block();
    // The default rules are seeded in one instant: Facebook, then Reels.
    let rules = app.store().rules().unwrap();
    let (facebook, reels) = (&rules[0], &rules[1]);
    assert_eq!(facebook.meta.created_at, reels.meta.created_at);
    assert!(facebook.meta.id < reels.meta.id);
    app.window.show("Reels | Facebook", "chrome.exe");

    app.poll_for(65);

    assert_eq!(app.events()[0].0, app.rule("Facebook"));
    assert_eq!(app.nudges()[0].0, "Facebook during your Learning block");
}

#[test]
fn an_event_left_open_by_a_crash_ends_at_the_last_heartbeat() {
    let mut app = App::new();
    let facebook = app.rule("Facebook");
    app.store()
        .insert_distraction(facebook, WatchContext::Office, ts("2026-10-22T02:59:00Z"))
        .unwrap();
    app.store().beat().unwrap(); // 03:00, then the app died
    app.clock.advance(SignedDuration::from_hours(2));

    app.poll();

    assert_eq!(app.events()[0].3, Some(ts("2026-10-22T03:00:00Z")));
}
