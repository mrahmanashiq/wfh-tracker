#![allow(clippy::unwrap_used)] // test code may unwrap (SPEC: Code Style)

use std::cell::RefCell;
use std::sync::Arc;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use jiff::{SignedDuration, Timestamp};
use wt_core::{
    CallAction, CategoryKind, CategoryRepo, FixedClock, Notifier, Placement, SessionRepo, Settings,
    Task, TaskRepo, TimerTarget,
};
use wt_storage::SqliteStore;
use wt_ui_egui::StatusBar;
use wt_ui_egui::{OfficeTone, office_tone};

/// Titles in `.0`, bodies in `.1`.
#[derive(Default)]
struct FakeNotifier(RefCell<Vec<String>>, RefCell<Vec<String>>);

impl Notifier for FakeNotifier {
    fn notify(&self, title: &str, body: &str) {
        self.0.borrow_mut().push(title.to_owned());
        self.1.borrow_mut().push(body.to_owned());
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
                .ui(ui, &s.store, &Settings::default(), s.clock.as_ref(), &[]);
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
            &[],
        );
    }

    fn running_kind(&self) -> Option<CategoryKind> {
        self.store.running().unwrap().map(|s| s.kind)
    }

    fn call(&mut self, action: CallAction) {
        self.bar.call_meeting(action, &self.store, &self.notifier);
    }

    fn toasts(&self) -> Vec<(String, String)> {
        let titles = self.notifier.0.borrow().clone();
        titles
            .into_iter()
            .zip(self.notifier.1.borrow().clone())
            .collect()
    }
}

fn toast(title: &str, body: &str) -> (String, String) {
    (title.to_owned(), body.to_owned())
}

#[test]
fn a_teams_call_pauses_the_task_for_a_meeting_and_resumes_it_after() {
    let mut h = harness();
    h.state().start_checkout();
    h.state_mut().call(CallAction::StartMeeting);
    assert_eq!(h.state().running_kind(), Some(CategoryKind::Meeting));

    h.state().later(30);
    h.state_mut().call(CallAction::EndMeeting);
    let running = h.state().store.running().unwrap().unwrap();
    assert_eq!(running.task_id, Some(h.state().checkout.meta.id));
    assert_eq!(
        h.state().toasts(),
        [
            toast(
                "Teams meeting detected",
                "Tracking it as a meeting. Checkout resumes when the call ends."
            ),
            toast("Teams meeting ended", "Back to Checkout."),
        ]
    );
}

#[test]
fn a_teams_call_with_nothing_running_just_tracks_the_meeting() {
    let mut h = harness();
    h.state_mut().call(CallAction::StartMeeting);
    h.state().later(30);
    h.state_mut().call(CallAction::EndMeeting);
    assert_eq!(h.state().running_kind(), None);
    assert_eq!(
        h.state().toasts(),
        [
            toast("Teams meeting detected", "Tracking it as a meeting."),
            toast("Teams meeting ended", "Meeting time saved."),
        ]
    );
}

#[test]
fn a_call_action_that_matches_the_timer_already_does_nothing() {
    let mut h = harness();
    h.state().start_checkout();
    // No meeting runs, so there's none to end: the task keeps going.
    h.state_mut().call(CallAction::EndMeeting);
    assert_eq!(h.state().running_kind(), Some(CategoryKind::Office));

    h.get_by_label("Meeting").click();
    h.run();
    // A meeting already runs: starting one must not toggle it off.
    h.state_mut().call(CallAction::StartMeeting);
    assert_eq!(h.state().running_kind(), Some(CategoryKind::Meeting));
    assert!(h.state().toasts().is_empty());
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
fn ending_a_meeting_restarts_the_interrupted_task() {
    let mut h = harness();
    h.state().start_checkout();
    h.run();

    h.get_by_label("Meeting").click();
    h.run();
    assert_eq!(h.state().running_kind(), Some(CategoryKind::Meeting));

    h.get_by_label("End meeting").click();
    h.run();

    // No click needed: ending the meeting restarts the interrupted task.
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
    h.get_by_label("Nothing running");
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

#[test]
fn the_running_timer_shows_the_tasks_total_for_today_not_just_this_stretch() {
    let mut h = harness();
    h.state().start_checkout();
    h.state().later(30);
    h.state().store.toggle_meeting().unwrap();
    h.state().later(10);
    h.state().start_checkout();
    h.state().later(5);
    h.run();

    h.get_by_label("00:35:00");
}

#[test]
fn the_office_meter_colours_follow_the_target_and_ceiling() {
    let settings = Settings::default(); // 8h target, 9h ceiling
    let thursday = jiff::civil::Weekday::Thursday;
    let tone = |minutes| office_tone(SignedDuration::from_mins(minutes), &settings, thursday);

    assert_eq!(tone(0), OfficeTone::Under);
    assert_eq!(tone(8 * 60 - 1), OfficeTone::Under);
    assert_eq!(tone(8 * 60), OfficeTone::Met);
    assert_eq!(tone(8 * 60 + 29), OfficeTone::Met);
    assert_eq!(tone(8 * 60 + 30), OfficeTone::NearCeiling);
    assert_eq!(tone(9 * 60 - 1), OfficeTone::NearCeiling);
    assert_eq!(tone(9 * 60), OfficeTone::Over);
    assert_eq!(tone(9 * 60 + 30), OfficeTone::Over);
}

#[test]
fn on_a_day_off_any_office_time_is_a_warning() {
    let settings = Settings::default();
    let friday = jiff::civil::Weekday::Friday;
    let tone = |minutes| office_tone(SignedDuration::from_mins(minutes), &settings, friday);

    assert_eq!(tone(0), OfficeTone::Under);
    assert_eq!(tone(10), OfficeTone::DayOff);
}
