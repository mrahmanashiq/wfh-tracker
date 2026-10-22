#![allow(clippy::unwrap_used)] // test code may unwrap (SPEC: Code Style)

use std::cell::RefCell;
use std::sync::Arc;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use jiff::{SignedDuration, Timestamp};
use wt_core::{
    CategoryKind, CategoryRepo, FixedClock, Notifier, Placement, SessionRepo, Settings, Task,
    TaskRepo, TimerTarget,
};
use wt_storage::SqliteStore;
use wt_ui_egui::StatusBar;

#[derive(Default)]
struct FakeNotifier(RefCell<Vec<String>>);

impl Notifier for FakeNotifier {
    fn notify(&self, title: &str, _body: &str) {
        self.0.borrow_mut().push(title.to_owned());
    }
}

struct State {
    store: SqliteStore,
    clock: Arc<FixedClock>,
    bar: StatusBar,
    notifier: FakeNotifier,
    checkout: Task,
}

fn harness() -> Harness<'static, State> {
    // 2026-10-22 is a Thursday, a workday.
    let clock = Arc::new(FixedClock::utc(
        "2026-10-22T03:00:00Z".parse::<Timestamp>().unwrap(),
    ));
    let store = SqliteStore::open_in_memory(clock.clone()).unwrap();
    store.seed_defaults().unwrap();
    let office = store.list().unwrap()[0].clone();
    let checkout = store
        .add(
            "Checkout",
            Placement::TopLevel {
                category_id: office.meta.id,
            },
        )
        .unwrap();
    Harness::new_ui_state(
        |ui, s: &mut State| {
            s.bar
                .ui(ui, &s.store, &Settings::default(), s.clock.as_ref());
            s.bar.quick_start(ui, &s.store);
        },
        State {
            store,
            clock,
            bar: StatusBar::default(),
            notifier: FakeNotifier::default(),
            checkout,
        },
    )
}

impl State {
    fn start_checkout(&self) {
        self.store
            .start(TimerTarget::Task(self.checkout.meta.id))
            .unwrap();
    }

    fn later(&self, minutes: i64) {
        self.clock.advance(SignedDuration::from_mins(minutes));
    }

    fn tick(&mut self) {
        self.bar.tick(
            &self.store,
            &Settings::default(),
            self.clock.as_ref(),
            &self.notifier,
        );
    }

    fn running_kind(&self) -> Option<CategoryKind> {
        self.store.running().unwrap().map(|s| s.kind)
    }
}

#[test]
fn shows_the_running_task_and_todays_office_time() {
    let mut h = harness();
    h.state().start_checkout();
    h.state().later(90);
    h.run();

    h.get_by_label("Checkout");
    h.get_by_label("Office 1h 30m / 8h");
}

#[test]
fn meeting_pauses_work_and_offers_to_resume_it_afterwards() {
    let mut h = harness();
    h.state().start_checkout();
    h.run();

    h.get_by_label("Meeting").click();
    h.run();
    assert_eq!(h.state().running_kind(), Some(CategoryKind::Meeting));

    h.get_by_label("End meeting").click();
    h.run();
    assert_eq!(h.state().running_kind(), None);

    h.get_by_label("Resume Checkout").click();
    h.run();
    let running = h.state().store.running().unwrap().unwrap();
    assert_eq!(running.task_id, Some(h.state().checkout.meta.id));
}

#[test]
fn a_break_can_be_taken_and_resumed_from() {
    let mut h = harness();
    h.state().start_checkout();
    h.run();

    h.get_by_label("Take break").click();
    h.run();
    assert_eq!(h.state().running_kind(), Some(CategoryKind::Break));

    h.get_by_label("Resume Checkout").click();
    h.run();
    assert_eq!(h.state().running_kind(), Some(CategoryKind::Office));
}

#[test]
fn quick_start_tracks_a_personal_category() {
    let mut h = harness();

    h.get_by_label("Start Family").click();
    h.run();

    let running = h.state().store.running().unwrap().unwrap();
    assert_eq!(
        (running.task_id, running.kind),
        (None, CategoryKind::Personal)
    );
}

#[test]
fn stop_ends_the_timer() {
    let mut h = harness();
    h.state().start_checkout();
    h.run();

    h.get_by_label("Stop").click();
    h.run();

    assert_eq!(h.state().running_kind(), None);
    h.get_by_label("No timer running");
}

#[test]
fn ticking_sends_a_break_reminder_as_a_notification() {
    let mut h = harness();
    h.state().start_checkout();

    // The app ticks every 15 s; once a simulated minute is enough here.
    // (One big jump would look like the app had been closed.)
    for _ in 0..25 {
        h.state_mut().tick();
        h.state().later(1);
    }
    h.state_mut().tick();

    assert_eq!(*h.state().notifier.0.borrow(), ["Time for a break"]);
}

#[test]
fn a_timer_left_running_while_the_app_was_closed_is_stopped_and_explained() {
    let mut h = harness();
    h.state().start_checkout();
    h.state().later(30);
    h.state_mut().tick(); // heartbeat at 03:30
    h.state().later(120);

    h.state_mut().tick();
    h.run();

    assert_eq!(h.state().running_kind(), None);
    h.get_by_label("Stopped Checkout at 03:30 while the app wasn't running.");
}

#[test]
fn a_meeting_started_with_nothing_running_offers_no_stale_resume() {
    let mut h = harness();
    h.state().start_checkout();
    h.run();
    h.get_by_label("Take break").click();
    h.run();
    h.state().store.stop().unwrap();
    h.run();

    h.get_by_label("Meeting").click();
    h.run();
    h.get_by_label("End meeting").click();
    h.run();

    assert!(h.query_by_label("Resume Checkout").is_none());
}

#[test]
fn on_a_day_off_all_office_time_shows_as_overtime() {
    let mut h = harness();
    h.state().later(24 * 60); // Thursday 03:00 → Friday 03:00
    h.state().start_checkout();
    h.state().later(90);
    h.run();

    h.get_by_label("Office 1h 30m · day off, 1h 30m overtime");
}
