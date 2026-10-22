//! Targets: how much time a category should get each week or month.

use std::fmt;
use std::str::FromStr;

use jiff::civil::{Date, Weekday};
use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp, ToSpan};

use crate::{Id, Meta, ParseError, Session};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Period {
    Week,
    Month,
}

impl Period {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Week => "week",
            Self::Month => "month",
        }
    }

    /// The most hours a target may ask for in one period.
    pub fn max_hours(self) -> u32 {
        match self {
            Self::Week => 200,
            Self::Month => 800,
        }
    }
}

impl FromStr for Period {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "week" => Ok(Self::Week),
            "month" => Ok(Self::Month),
            _ => Err(ParseError::new(format!("unknown period `{s}`"))),
        }
    }
}

impl fmt::Display for Period {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How much time a category should get each week or month.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub meta: Meta,
    pub category_id: Id,
    pub period: Period,
    pub minutes: u32,
}

/// The local dates `[start, end)` of the week or month containing `day`.
pub fn period_range(day: Date, period: Period, week_start: Weekday) -> (Date, Date) {
    match period {
        Period::Week => {
            let back = (day.weekday().to_sunday_zero_offset() - week_start.to_sunday_zero_offset())
                .rem_euclid(7);
            let start = day.checked_sub(i64::from(back).days()).unwrap_or(day);
            (start, start.checked_add(7.days()).unwrap_or(start))
        }
        Period::Month => {
            let start = day.first_of_month();
            (start, start.checked_add(1.month()).unwrap_or(start))
        }
    }
}

/// Time tracked in the target's category during the current period,
/// counting a running session up to `now`.
pub fn target_progress(
    sessions: &[Session],
    target: &Target,
    today: Date,
    tz: &TimeZone,
    now: Timestamp,
    week_start: Weekday,
) -> SignedDuration {
    let (first, after) = period_range(today, target.period, week_start);
    let instant = |d: Date| d.to_zoned(tz.clone()).map(|z| z.timestamp());
    let (Ok(start), Ok(end)) = (instant(first), instant(after)) else {
        return SignedDuration::ZERO;
    };
    sessions
        .iter()
        .filter(|s| s.category_id == target.category_id)
        .map(|s| {
            let (from, to) = (s.started_at.max(start), s.end_or(now).min(end));
            if to > from {
                to.duration_since(from)
            } else {
                SignedDuration::ZERO
            }
        })
        .fold(SignedDuration::ZERO, SignedDuration::saturating_add)
}
