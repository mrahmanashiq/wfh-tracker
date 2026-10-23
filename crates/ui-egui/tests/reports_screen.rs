#![allow(clippy::unwrap_used)] // test code may unwrap (SPEC: Code Style)

use std::cell::RefCell;
use std::sync::Arc;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use jiff::{SignedDuration, Timestamp};
use wt_core::{
    CategoryRepo, Clipboard, ClipboardError, DistractionRepo, FixedClock, Placement, SessionRepo,
    Settings, TaskRepo, TimerTarget, WatchContext,
};
use wt_storage::SqliteStore;
use wt_ui_egui::ReportsScreen;

#[derive(Default)]
struct FakeClipboard(RefCell<Option<String>>);

impl Clipboard for FakeClipboard {
    fn set_text(&self, text: &str) -> Result<(), ClipboardError> {
        *self.0.borrow_mut() = Some(text.to_owned());
        Ok(())
    }
}

struct State {
    store: SqliteStore,
    clock: Arc<FixedClock>,
    clipboard: FakeClipboard,
    screen: ReportsScreen,
}

/// Thursday 22 Oct 2026: an hour on "Checkout", then 20 minutes of family time.
fn harness() -> Harness<'static, State> {
    let clock = Arc::new(FixedClock::utc(
        "2026-10-22T03:00:00Z".parse::<Timestamp>().unwrap(),
    ));
    let store = SqliteStore::open_in_memory(clock.clone()).unwrap();
    store.seed_defaults().unwrap();
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
    store.start(TimerTarget::Task(checkout.meta.id)).unwrap();
    clock.advance(SignedDuration::from_mins(60));
    store.start(TimerTarget::Category(id_of("Family"))).unwrap();
    clock.advance(SignedDuration::from_mins(20));
    store.stop().unwrap();

    // The app adds an emoji font for ✅ ⏳; the bare test fonts lack them.
    let mut h = Harness::builder().allow_missing_glyphs().build_ui_state(
        |ui, s: &mut State| {
            s.screen.ui(
                ui,
                &s.store,
                &Settings::default(),
                s.clock.as_ref(),
                &s.clipboard,
            );
        },
        State {
            store,
            clock,
            clipboard: FakeClipboard::default(),
            screen: ReportsScreen::default(),
        },
    );
    h.run();
    h
}

fn copied(h: &Harness<'_, State>) -> String {
    h.state().clipboard.0.borrow().clone().unwrap()
}

#[test]
fn copy_office_report_puts_the_report_on_the_clipboard() {
    let mut h = harness();

    h.get_by_label("Copy office report").click();
    h.run();

    assert_eq!(
        copied(&h),
        "Office report - Thu 22 Oct 2026 - 1h 00m (target 8h)\n\
         \n\
         ⏳ Checkout - 1h 00m\n\
         Meetings - none\n\
         Overtime - none"
    );
    h.get_by_label("Copied");
}

#[test]
fn copy_my_time_puts_the_private_report_on_the_clipboard() {
    let mut h = harness();

    h.get_by_label("Copy my time").click();
    h.run();

    assert_eq!(
        copied(&h),
        "My time - Thu 22 Oct 2026\n\nPersonal - 0h 20m\n  Family - 20m"
    );
}

#[test]
fn the_previous_day_button_shows_yesterdays_reports() {
    let mut h = harness();

    h.get_by_label("Previous day").click();
    h.run();
    h.get_by_label("Copy office report").click();
    h.run();

    assert_eq!(
        copied(&h),
        "Office report - Wed 21 Oct 2026 - 0h 00m (target 8h)\n\
         \n\
         Meetings - none\n\
         Overtime - none"
    );
}

#[test]
fn my_time_includes_the_days_distractions_but_the_office_report_does_not() {
    let mut h = harness();
    let store = &h.state().store;
    store.seed_default_rules().unwrap();
    let facebook = store.rules().unwrap()[0].meta.id;
    // 03:10-03:25, during the hour on "Checkout".
    store
        .insert_distraction(
            facebook,
            WatchContext::Office,
            "2026-10-22T03:10:00Z".parse().unwrap(),
        )
        .unwrap();
    store
        .end_distractions("2026-10-22T03:25:00Z".parse().unwrap())
        .unwrap();
    h.run();

    h.get_by_label("Copy my time").click();
    h.run();
    assert_eq!(
        copied(&h),
        "My time - Thu 22 Oct 2026\n\
         \n\
         Personal - 0h 20m\n  \
         Family - 20m\n\
         \n\
         Distractions - 0h 15m\n  \
         Facebook - 15m (office)"
    );

    h.get_by_label("Copy office report").click();
    h.run();
    assert!(!copied(&h).contains("Facebook"));
}
