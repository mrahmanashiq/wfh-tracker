#![allow(clippy::unwrap_used)] // test code may unwrap (SPEC: Code Style)

use std::cell::{Cell, RefCell};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use jiff::civil::Weekday::*;
use wt_core::repo::RepoResult;
use wt_core::{Appearance, Autostart, AutostartError, Settings, SettingsRepo};
use wt_ui_egui::SettingsScreen;

#[derive(Default)]
struct FakeSettings(RefCell<Settings>);

impl SettingsRepo for FakeSettings {
    fn load(&self) -> RepoResult<Settings> {
        Ok(self.0.borrow().clone())
    }

    fn save(&self, settings: &Settings) -> RepoResult<()> {
        settings.validate()?;
        *self.0.borrow_mut() = settings.clone();
        Ok(())
    }
}

struct FakeAutostart(Cell<bool>);

impl Autostart for FakeAutostart {
    fn is_enabled(&self) -> Result<bool, AutostartError> {
        Ok(self.0.get())
    }

    fn set_enabled(&self, enabled: bool) -> Result<(), AutostartError> {
        self.0.set(enabled);
        Ok(())
    }
}

struct State {
    repo: FakeSettings,
    autostart: FakeAutostart,
    screen: SettingsScreen,
}

fn harness() -> Harness<'static, State> {
    let repo = FakeSettings::default();
    let autostart = FakeAutostart(Cell::new(true));
    let screen = SettingsScreen::load(&repo);
    Harness::new_ui_state(
        |ui, s: &mut State| s.screen.ui(ui, &s.repo, &s.autostart),
        State {
            repo,
            autostart,
            screen,
        },
    )
}

#[test]
fn edited_office_hours_are_saved() {
    let mut h = harness();

    h.get_by_label("Office target (min)")
        .set_value_accesskit(450.0);
    h.get_by_label("Soft ceiling (min)")
        .set_value_accesskit(510.0);
    h.run();
    h.get_by_label("Save").click();
    h.run();

    let saved = h.state().repo.0.borrow().clone();
    assert_eq!(
        (saved.office_target_min, saved.office_ceiling_min),
        (450, 510)
    );
    h.get_by_label("Saved.");
}

#[test]
fn ticking_friday_makes_it_a_workday() {
    let mut h = harness();

    h.get_by_label("Fri").click();
    h.run();
    h.get_by_label("Save").click();
    h.run();

    assert_eq!(
        h.state().repo.0.borrow().workdays,
        [Sunday, Monday, Tuesday, Wednesday, Thursday, Friday]
    );
}

#[test]
fn turning_off_start_at_login_disables_autostart() {
    let mut h = harness();

    h.get_by_label("Start at login").click();
    h.run();
    h.get_by_label("Save").click();
    h.run();

    assert!(!h.state().autostart.0.get());
    assert!(!h.state().repo.0.borrow().autostart);
}

#[test]
fn ceiling_below_target_shows_the_error_and_saves_nothing() {
    let mut h = harness();

    h.get_by_label("Soft ceiling (min)")
        .set_value_accesskit(400.0);
    h.run();
    h.get_by_label("Save").click();
    h.run();

    h.get_by_label("office soft ceiling must be at least the office target");
    assert_eq!(*h.state().repo.0.borrow(), Settings::default());
}

#[test]
fn appearance_defaults_to_light_and_dark_can_be_chosen() {
    let mut h = harness();
    assert_eq!(h.state().repo.load().unwrap().appearance, Appearance::Light);

    h.get_by_label("Dark").click();
    h.run();
    h.get_by_label("Save").click();
    h.run();

    assert_eq!(h.state().repo.load().unwrap().appearance, Appearance::Dark);
}

#[test]
fn teams_call_tracking_is_on_by_default_and_can_be_switched_off() {
    let mut h = harness();
    assert!(h.state().repo.load().unwrap().detect_teams_calls);

    h.get_by_label("Track Teams calls as meetings").click();
    h.run();
    h.get_by_label("Save").click();
    h.run();

    assert!(!h.state().repo.load().unwrap().detect_teams_calls);
}
