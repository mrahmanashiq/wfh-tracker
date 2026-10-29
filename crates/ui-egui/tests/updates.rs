#![allow(clippy::unwrap_used)] // test code may unwrap (SPEC: Code Style)

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use wt_core::{AvailableUpdate, UpdateError, Updater};
use wt_ui_egui::UpdateController;

/// Pretends to be GitHub: offers `next` (if any) and records what was asked.
struct FakeUpdater {
    next: Option<&'static str>,
    fail: bool,
    checks: AtomicUsize,
    installed: AtomicBool,
}

impl FakeUpdater {
    fn offering(next: Option<&'static str>) -> Arc<Self> {
        Arc::new(Self {
            next,
            fail: false,
            checks: AtomicUsize::new(0),
            installed: AtomicBool::new(false),
        })
    }

    fn failing() -> Arc<Self> {
        Arc::new(Self {
            next: None,
            fail: true,
            checks: AtomicUsize::new(0),
            installed: AtomicBool::new(false),
        })
    }
}

impl Updater for FakeUpdater {
    fn current_version(&self) -> &str {
        "0.1.0"
    }

    fn check(&self) -> Result<Option<AvailableUpdate>, UpdateError> {
        self.checks.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            return Err(UpdateError("no network".to_owned()));
        }
        Ok(self.next.map(|v| AvailableUpdate {
            version: v.to_owned(),
        }))
    }

    fn download(&self) -> Result<(), UpdateError> {
        Ok(())
    }

    fn install_and_relaunch(&self) -> Result<(), UpdateError> {
        self.installed.store(true, Ordering::SeqCst);
        Ok(())
    }
}

fn harness(updater: Arc<FakeUpdater>) -> Harness<'static, UpdateController> {
    Harness::new_ui_state(
        |ui, c: &mut UpdateController| {
            c.header_ui(ui);
            c.settings_ui(ui);
        },
        UpdateController::new(Some(updater)),
    )
}

fn header_harness(updater: Arc<FakeUpdater>) -> Harness<'static, UpdateController> {
    Harness::new_ui_state(
        |ui, c: &mut UpdateController| c.header_ui(ui),
        UpdateController::new(Some(updater)),
    )
}

/// Runs a check and waits for its background thread to finish.
fn check(h: &mut Harness<'_, UpdateController>) {
    h.get_by_label("Check for updates").click();
    h.run();
    h.state_mut().finish_background_work();
    h.run();
}

#[test]
fn shows_the_installed_version() {
    let h = harness(FakeUpdater::offering(None));

    h.get_by_label("Version 0.1.0");
}

#[test]
fn the_manual_check_is_only_in_settings() {
    let h = header_harness(FakeUpdater::offering(None));

    assert!(h.query_by_label("Check for updates").is_none());
}

#[test]
fn a_newer_version_is_downloaded_then_offered_as_a_restart() {
    let updater = FakeUpdater::offering(Some("0.2.0"));
    let mut h = harness(updater.clone());

    check(&mut h);
    h.get_by_label("Restart to update to 0.2.0").click();
    h.run();

    assert!(updater.installed.load(Ordering::SeqCst));
}

#[test]
fn being_up_to_date_is_said_plainly() {
    let mut h = harness(FakeUpdater::offering(None));

    check(&mut h);

    h.get_by_label("You're up to date.");
    assert!(h.query_by_label("Restart to update to 0.2.0").is_none());
}

#[test]
fn a_failed_check_explains_itself() {
    let mut h = harness(FakeUpdater::failing());

    check(&mut h);

    h.get_by_label("Couldn't check for updates: no network");
}

#[test]
fn the_daily_check_runs_once_on_start_and_not_again_soon_after() {
    let updater = FakeUpdater::offering(None);
    let mut controller = UpdateController::new(Some(updater.clone()));
    let ctx = egui::Context::default();

    controller.tick(&ctx);
    controller.finish_background_work();
    controller.tick(&ctx);
    controller.finish_background_work();

    assert_eq!(updater.checks.load(Ordering::SeqCst), 1);
}

#[test]
fn development_builds_have_no_updater_and_say_so() {
    let h = Harness::new_ui_state(
        |ui, c: &mut UpdateController| c.settings_ui(ui),
        UpdateController::new(None),
    );

    h.get_by_label("Updates are off in development builds.");
}
