//! Distraction rules ("Facebook in the window title") and the distraction
//! events recorded when one matched for longer than the grace period.
//! Only rule ids and times are ever stored, never window titles.

use std::fmt;
use std::str::FromStr;

use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp};

use crate::session::span_within;
use crate::{ForegroundWindow, Id, Meta, ParseError, RepoError};

/// Which part of the foreground window a rule looks at.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MatchOn {
    Title,
    Process,
}

impl MatchOn {
    pub const ALL: [Self; 2] = [Self::Title, Self::Process];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Process => "process",
        }
    }
}

impl FromStr for MatchOn {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|m| m.as_str() == s)
            .ok_or_else(|| ParseError::new(format!("unknown match target `{s}`")))
    }
}

/// A window the user doesn't want in front during protected time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DistractionRule {
    pub meta: Meta,
    /// Matched as a case-insensitive substring.
    pub pattern: String,
    pub match_on: MatchOn,
    pub enabled: bool,
}

impl DistractionRule {
    /// Whether this (enabled) rule matches the window.
    pub fn matches(&self, window: &ForegroundWindow) -> bool {
        let haystack = match self.match_on {
            MatchOn::Title => &window.title,
            MatchOn::Process => &window.process,
        };
        self.enabled
            && haystack
                .to_lowercase()
                .contains(&self.pattern.to_lowercase())
    }
}

pub const MAX_PATTERN_CHARS: usize = 60;

/// The editable fields of a rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleDraft {
    pub pattern: String,
    pub match_on: MatchOn,
}

impl RuleDraft {
    pub fn new(pattern: &str, match_on: MatchOn) -> Self {
        Self {
            pattern: pattern.to_owned(),
            match_on,
        }
    }

    /// Checks the rules and returns the draft with its pattern trimmed.
    pub fn validated(&self) -> Result<Self, RepoError> {
        let pattern = self.pattern.trim();
        if pattern.is_empty() {
            return Err(RepoError::Invalid("a rule needs some text to match"));
        }
        if pattern.chars().count() > MAX_PATTERN_CHARS {
            return Err(RepoError::Invalid(
                "a rule's text can be at most 60 characters",
            ));
        }
        Ok(Self::new(pattern, self.match_on))
    }

    /// The rules a fresh install starts with.
    pub fn defaults() -> Vec<Self> {
        ["Facebook", "Reels", "Shorts", "Instagram"]
            .into_iter()
            .map(|p| Self::new(p, MatchOn::Title))
            .collect()
    }
}

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
