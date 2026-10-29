#![allow(clippy::unwrap_used)] // test code may unwrap (SPEC: Code Style)

use std::sync::Arc;

use jiff::{SignedDuration, Timestamp};
use wt_core::{
    Category, CategoryKind, CategoryRepo, FixedClock, MeetingToggle, Placement, RepoError, Session,
    SessionRepo, Task, TaskRepo, TimerTarget,
};
use wt_storage::SqliteStore;

fn at(ts: &str) -> Timestamp {
    ts.parse().unwrap()
}

struct World {
    store: SqliteStore,
    clock: Arc<FixedClock>,
    family: Category,
    checkout: Task,
    api: Task,
}

fn world() -> World {
    let clock = Arc::new(FixedClock::utc(at("2026-10-22T03:00:00Z")));
    let store = SqliteStore::open_in_memory(clock.clone()).unwrap();
    store.seed_defaults().unwrap();
    let cats = store.list().unwrap();
    let office = cats.iter().find(|c| c.name == "Office").unwrap().clone();
    let family = cats.iter().find(|c| c.name == "Family").unwrap().clone();
    let checkout = store
        .add(
            "Checkout",
            Placement::TopLevel {
                category_id: office.meta.id,
            },
        )
        .unwrap();
    let api = store
        .add("API", Placement::Under(checkout.meta.id))
        .unwrap();
    World {
        store,
        clock,
        family,
        checkout,
        api,
    }
}

impl World {
    fn later(&self, minutes: i64) {
        self.clock.advance(SignedDuration::from_mins(minutes));
    }

    fn all(&self) -> Vec<Session> {
        self.store
            .sessions_since(at("2026-10-01T00:00:00Z"))
            .unwrap()
    }

    fn running(&self) -> Option<Session> {
        self.store.running().unwrap()
    }
}

#[test]
fn starting_another_task_leaves_the_first_task_running() {
    let w = world();
    w.store
        .start(TimerTarget::Task(w.checkout.meta.id))
        .unwrap();
    w.later(30);

    w.store.start(TimerTarget::Task(w.api.meta.id)).unwrap();

    let all = w.all();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].ended_at, None);
    assert_eq!(all[1].started_at, at("2026-10-22T03:30:00Z"));
    assert_eq!(all.iter().filter(|s| s.is_running()).count(), 2);
}

#[test]
fn a_task_session_takes_the_tasks_category_and_its_kind() {
    let w = world();

    let s = w.store.start(TimerTarget::Task(w.api.meta.id)).unwrap();

    assert_eq!(s.category_id, w.checkout.category_id);
    assert_eq!(s.kind, CategoryKind::Office);
}

#[test]
fn quick_start_tracks_a_category_without_a_task() {
    let w = world();

    let s = w
        .store
        .start(TimerTarget::Category(w.family.meta.id))
        .unwrap();

    assert_eq!((s.task_id, s.category_id), (None, w.family.meta.id));
    assert_eq!(s.kind, CategoryKind::Personal);
}

#[test]
fn stop_ends_the_running_session_and_is_a_no_op_when_idle() {
    let w = world();
    w.store.start(TimerTarget::Task(w.api.meta.id)).unwrap();
    w.later(10);

    let stopped = w.store.stop().unwrap().unwrap();

    assert_eq!(stopped.ended_at, Some(at("2026-10-22T03:10:00Z")));
    assert!(w.running().is_none());
    assert!(w.store.stop().unwrap().is_none());
}

#[test]
fn the_meeting_toggle_pauses_work_and_hands_back_what_to_resume() {
    let w = world();
    let work = w.store.start(TimerTarget::Task(w.api.meta.id)).unwrap();
    w.later(20);

    let started = w.store.toggle_meeting().unwrap();
    let MeetingToggle::Started { paused } = started else {
        panic!("expected a meeting to start, got {started:?}");
    };
    assert_eq!(paused.map(|s| s.meta.id), Some(work.meta.id));
    assert_eq!(w.running().unwrap().kind, CategoryKind::Meeting);

    w.later(40);
    let ended = w.store.toggle_meeting().unwrap();
    assert!(matches!(ended, MeetingToggle::Ended { .. }), "{ended:?}");
}

#[test]
fn ending_a_meeting_restarts_the_task_it_interrupted_at_the_same_instant() {
    let w = world();
    w.store.start(TimerTarget::Task(w.api.meta.id)).unwrap();
    w.later(20);
    w.store.toggle_meeting().unwrap();
    w.later(40);

    let ended = w.store.toggle_meeting().unwrap();

    let MeetingToggle::Ended { meeting, resumed } = ended else {
        panic!("expected the meeting to end, got {ended:?}");
    };
    let resumed = resumed.expect("the task should restart");
    assert_eq!(resumed.task_id, Some(w.api.meta.id));
    assert_eq!(Some(resumed.started_at), meeting.ended_at);
    assert_eq!(w.running().unwrap().task_id, Some(w.api.meta.id));
}

#[test]
fn a_meeting_started_while_idle_restarts_nothing() {
    let w = world();
    w.store.toggle_meeting().unwrap();
    w.later(30);

    let ended = w.store.toggle_meeting().unwrap();

    assert!(
        matches!(ended, MeetingToggle::Ended { resumed: None, .. }),
        "{ended:?}"
    );
    assert!(w.running().is_none());
}

#[test]
fn a_task_deleted_during_the_meeting_is_not_restarted() {
    let w = world();
    w.store.start(TimerTarget::Task(w.api.meta.id)).unwrap();
    w.later(20);
    w.store.toggle_meeting().unwrap();
    TaskRepo::delete(&w.store, w.api.meta.id).unwrap();
    w.later(40);

    let ended = w.store.toggle_meeting().unwrap();

    assert!(
        matches!(ended, MeetingToggle::Ended { resumed: None, .. }),
        "{ended:?}"
    );
    assert!(w.running().is_none());
}

#[test]
fn taking_a_break_ends_work_and_runs_a_break_session() {
    let w = world();
    w.store.start(TimerTarget::Task(w.api.meta.id)).unwrap();
    w.later(25);

    w.store.take_break().unwrap();

    let all = w.all();
    assert_eq!(all[0].ended_at, Some(at("2026-10-22T03:25:00Z")));
    assert_eq!(w.running().unwrap().kind, CategoryKind::Break);
}

#[test]
fn meetings_need_a_meeting_category() {
    let w = world();
    let meeting = w
        .store
        .list()
        .unwrap()
        .into_iter()
        .find(|c| c.kind == CategoryKind::Meeting)
        .unwrap();
    CategoryRepo::delete(&w.store, meeting.meta.id).unwrap();

    assert!(matches!(
        w.store.toggle_meeting(),
        Err(RepoError::Invalid(_))
    ));
}

#[test]
fn a_timer_left_running_across_a_heartbeat_gap_ends_at_the_last_heartbeat() {
    let w = world();
    w.store.start(TimerTarget::Task(w.api.meta.id)).unwrap();
    w.later(30);
    w.store.beat().unwrap();

    w.later(120); // app closed or PC asleep
    let recovered = w
        .store
        .recover(w.clock_now(), SignedDuration::from_mins(3))
        .unwrap();

    assert_eq!(
        recovered.unwrap().ended_at,
        Some(at("2026-10-22T03:30:00Z"))
    );
    assert!(w.running().is_none());
}

#[test]
fn a_recent_heartbeat_leaves_the_timer_running() {
    let w = world();
    w.store.start(TimerTarget::Task(w.api.meta.id)).unwrap();
    w.later(30);
    w.store.beat().unwrap();
    w.later(1);

    let recovered = w
        .store
        .recover(w.clock_now(), SignedDuration::from_mins(3))
        .unwrap();

    assert!(recovered.is_none());
    assert!(w.running().is_some());
}

#[test]
fn a_task_can_carry_its_own_break_interval() {
    let w = world();

    w.store
        .set_break_interval(w.checkout.meta.id, Some(50))
        .unwrap();

    assert_eq!(
        w.store.get(w.checkout.meta.id).unwrap().break_interval_min,
        Some(50)
    );
    assert_eq!(w.store.get(w.api.meta.id).unwrap().break_interval_min, None);
}

impl World {
    fn clock_now(&self) -> Timestamp {
        use wt_core::Clock;
        self.clock.now()
    }
}
