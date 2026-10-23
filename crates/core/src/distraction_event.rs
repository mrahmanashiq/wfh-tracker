//! Distraction events: the stretches of time a rule matched for longer than
//! the grace period. Only the rule id, context and times are stored.

use std::fmt;
use std::str::FromStr;

use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp};

use crate::session::span_within;
use crate::{Id, Meta, ParseError};

/// What the user was supposed to be doing when a distraction happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum WatchContext {
    Learning,
    Office,
}

impl WatchContext {
    pub const ALL: [Self; 2] = [Self::Learning, Self::Office];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Learning => "learning",
            Self::Office => "office",
        }
    }
}

impl fmt::Display for WatchContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for WatchContext {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|c| c.as_str() == s)
            .ok_or_else(|| ParseError::new(format!("unknown watch context `{s}`")))
    }
}

/// A stretch of time a rule matched past the grace period. `ended_at` is
/// `None` while it lasts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DistractionEvent {
    pub meta: Meta,
    pub rule_id: Id,
    pub context: WatchContext,
    pub started_at: Timestamp,
    pub ended_at: Option<Timestamp>,
}

impl DistractionEvent {
    /// How much of this event falls on the local `day`, an open one counting
    /// up to `now`.
    pub fn time_within(&self, day: Date, tz: &TimeZone, now: Timestamp) -> SignedDuration {
        span_within(self.started_at, self.ended_at.unwrap_or(now), day, tz)
    }
}
