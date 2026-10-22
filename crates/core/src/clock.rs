use std::sync::Mutex;

use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp};

/// The only source of "now" — and of the user's time zone — for domain logic.
pub trait Clock: Send + Sync {
    fn now(&self) -> Timestamp;
    fn time_zone(&self) -> TimeZone;

    /// The user's local calendar day.
    fn today(&self) -> Date {
        self.now().to_zoned(self.time_zone()).date()
    }
}

/// A clock that stays where you put it. For tests and simulations.
#[derive(Debug)]
pub struct FixedClock {
    now: Mutex<Timestamp>,
    tz: TimeZone,
}

impl FixedClock {
    pub fn new(at: Timestamp, tz: TimeZone) -> Self {
        Self {
            now: Mutex::new(at),
            tz,
        }
    }

    pub fn utc(at: Timestamp) -> Self {
        Self::new(at, TimeZone::UTC)
    }

    pub fn advance(&self, by: SignedDuration) {
        let mut now = self.now.lock().unwrap_or_else(|e| e.into_inner());
        *now = now.saturating_add(by).unwrap_or(*now);
    }
}

impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        *self.now.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn time_zone(&self) -> TimeZone {
        self.tz.clone()
    }
}
