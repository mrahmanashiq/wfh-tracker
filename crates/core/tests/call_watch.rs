#![allow(clippy::unwrap_used)] // test code may unwrap (SPEC: Code Style)

use jiff::{SignedDuration, Timestamp};
use wt_core::{CallAction, CallWatch};

const T0: &str = "2026-10-27T09:00:00Z";

/// Plays `samples` of (seconds after T0, mic in use) with the meeting timer
/// following the watch's own actions, and returns the actions with their times.
fn play(meeting_at_start: bool, samples: &[(i64, bool)]) -> Vec<(i64, CallAction)> {
    let t0: Timestamp = T0.parse().unwrap();
    let mut watch = CallWatch::default();
    let mut meeting = meeting_at_start;
    let mut actions = Vec::new();
    for &(secs, mic) in samples {
        let now = t0 + SignedDuration::from_secs(secs);
        if let Some(action) = watch.observe(mic, meeting, now) {
            meeting = action == CallAction::StartMeeting;
            actions.push((secs, action));
        }
    }
    actions
}

/// A sample every 5 seconds from `from` to `to` (inclusive) with the mic `on`.
fn every5(from: i64, to: i64, on: bool) -> Vec<(i64, bool)> {
    (from..=to).step_by(5).map(|s| (s, on)).collect()
}

#[test]
fn a_call_starts_a_meeting_after_20s_and_ends_it_15s_after_the_mic_frees() {
    let samples = [every5(0, 300, true), every5(305, 330, false)].concat();
    assert_eq!(
        play(false, &samples),
        [
            (20, CallAction::StartMeeting),
            (320, CallAction::EndMeeting)
        ]
    );
}

#[test]
fn a_short_mic_test_starts_nothing() {
    let samples = [every5(0, 15, true), every5(20, 60, false)].concat();
    assert_eq!(play(false, &samples), []);
}

#[test]
fn a_brief_mic_drop_mid_call_keeps_the_meeting() {
    let samples = [
        every5(0, 100, true),
        every5(105, 110, false),
        every5(115, 200, true),
        every5(205, 220, false),
    ]
    .concat();
    assert_eq!(
        play(false, &samples),
        [
            (20, CallAction::StartMeeting),
            (220, CallAction::EndMeeting)
        ]
    );
}

#[test]
fn a_meeting_already_running_is_adopted_and_ended_with_the_call() {
    let samples = [every5(0, 100, true), every5(105, 120, false)].concat();
    assert_eq!(play(true, &samples), [(120, CallAction::EndMeeting)]);
}

#[test]
fn a_meeting_the_user_ends_mid_call_is_not_restarted() {
    let t0: Timestamp = T0.parse().unwrap();
    let at = |s| t0 + SignedDuration::from_secs(s);
    let mut watch = CallWatch::default();
    assert_eq!(watch.observe(true, false, at(0)), None);
    assert_eq!(
        watch.observe(true, false, at(20)),
        Some(CallAction::StartMeeting)
    );
    // The user pressed Meeting (or started a task): no meeting runs now.
    for s in (25..=200).step_by(5) {
        assert_eq!(watch.observe(true, false, at(s)), None);
    }
    for s in (205..=240).step_by(5) {
        assert_eq!(watch.observe(false, false, at(s)), None);
    }
    // The next call starts a meeting again.
    assert_eq!(watch.observe(true, false, at(300)), None);
    assert_eq!(
        watch.observe(true, false, at(320)),
        Some(CallAction::StartMeeting)
    );
}

#[test]
fn reset_forgets_the_call() {
    let t0: Timestamp = T0.parse().unwrap();
    let at = |s| t0 + SignedDuration::from_secs(s);
    let mut watch = CallWatch::default();
    watch.observe(true, false, at(0));
    assert_eq!(
        watch.observe(true, false, at(20)),
        Some(CallAction::StartMeeting)
    );
    watch.reset();
    // A fresh watch neither ends that meeting nor restarts one straight away.
    assert_eq!(watch.observe(false, true, at(25)), None);
    assert_eq!(watch.observe(false, true, at(60)), None);
}
