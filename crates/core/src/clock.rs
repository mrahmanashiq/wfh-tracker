use std::sync::Mutex;

use jiff::{SignedDuration, Timestamp};

/// The only source of "now" for domain logic.
pub trait Clock: Send + Sync {
    fn now(&self) -> Timestamp;
}

/// A clock that stays where you put it. For tests and simulations.
#[derive(Debug)]
pub struct FixedClock(Mutex<Timestamp>);

impl FixedClock {
    pub fn new(at: Timestamp) -> Self {
        Self(Mutex::new(at))
    }

    pub fn advance(&self, by: SignedDuration) {
        let mut now = self.0.lock().unwrap_or_else(|e| e.into_inner());
        *now = now.saturating_add(by).unwrap_or(*now);
    }
}

impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        *self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}
