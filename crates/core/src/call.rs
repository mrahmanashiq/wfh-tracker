use jiff::{SignedDuration, Timestamp};

/// Tells whether a Teams call holds the microphone right now. Teams keeps the
/// mic open for the whole call, muted or not, so this is the call signal.
/// `false` when it can't be read.
pub trait CallSensor {
    fn in_call(&self) -> bool;
}

/// How long the mic must stay in use before it counts as a call, so a mic
/// test or a call preview doesn't start a meeting.
pub const CALL_START_AFTER: SignedDuration = SignedDuration::from_secs(20);
/// How long the mic must stay free before the call counts as over, so a
/// headset switch mid-call doesn't end the meeting.
pub const CALL_END_AFTER: SignedDuration = SignedDuration::from_secs(15);

/// What the app should do about the meeting timer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallAction {
    StartMeeting,
    EndMeeting,
}

/// Turns mic samples into meeting starts and ends. A call that begins while
/// a meeting is already running adopts it. Only a meeting this watch started
/// or adopted is ended by it: once the user ends or replaces that meeting
/// during the call, nothing restarts until the call is over.
#[derive(Clone, Debug, Default)]
pub struct CallWatch {
    /// When the current stretch of mic use (or of a free mic) began.
    since: Option<(bool, Timestamp)>,
    in_call: bool,
    /// The running meeting belongs to the current call.
    owns: bool,
}

impl CallWatch {
    /// Feeds one sample. `meeting_running` is whether a meeting timer runs now.
    pub fn observe(
        &mut self,
        mic_in_use: bool,
        meeting_running: bool,
        now: Timestamp,
    ) -> Option<CallAction> {
        let since = match self.since {
            Some((state, at)) if state == mic_in_use => at,
            _ => {
                self.since = Some((mic_in_use, now));
                now
            }
        };
        let held = now.duration_since(since);
        if self.owns && !meeting_running {
            // Ended or replaced by the user: leave it until the call is over.
            self.owns = false;
        }
        if !self.in_call {
            if mic_in_use && held >= CALL_START_AFTER {
                self.in_call = true;
                self.owns = true;
                return (!meeting_running).then_some(CallAction::StartMeeting);
            }
        } else if !mic_in_use && held >= CALL_END_AFTER {
            self.in_call = false;
            let owned = std::mem::take(&mut self.owns);
            return (owned && meeting_running).then_some(CallAction::EndMeeting);
        }
        None
    }

    /// Forgets the current call, e.g. when detection is switched off.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}
