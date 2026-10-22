//! Scheduled blocks: recurring local time reserved for one category.

use jiff::Timestamp;
use jiff::Zoned;
use jiff::civil::Weekday;
use jiff::tz::TimeZone;

use crate::{Id, Meta, RepoError};

/// A set of weekdays, stored as a bitmask (Sunday = bit 0 … Saturday = bit 6).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Weekdays(u8);

impl Weekdays {
    pub const ALL_IN_ORDER: [Weekday; 7] = [
        Weekday::Sunday,
        Weekday::Monday,
        Weekday::Tuesday,
        Weekday::Wednesday,
        Weekday::Thursday,
        Weekday::Friday,
        Weekday::Saturday,
    ];

    pub fn from_days(days: &[Weekday]) -> Self {
        Self(days.iter().fold(0, |bits, d| bits | bit(*d)))
    }

    pub fn from_bits(bits: u8) -> Self {
        Self(bits & 0x7f)
    }

    pub fn bits(self) -> u8 {
        self.0
    }

    pub fn contains(self, day: Weekday) -> bool {
        self.0 & bit(day) != 0
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn toggled(self, day: Weekday) -> Self {
        Self(self.0 ^ bit(day))
    }

    /// The days in Sunday-first order.
    pub fn days(self) -> impl Iterator<Item = Weekday> {
        Self::ALL_IN_ORDER
            .into_iter()
            .filter(move |d| self.contains(*d))
    }
}

fn bit(day: Weekday) -> u8 {
    1 << day.to_sunday_zero_offset()
}

/// A recurring stretch of local time reserved for one category.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub meta: Meta,
    pub category_id: Id,
    pub weekdays: Weekdays,
    /// Local start and end, in minutes after midnight; `end_min > start_min`.
    pub start_min: u16,
    pub end_min: u16,
    pub label: String,
}

/// The editable fields of a block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockDraft {
    pub category_id: Id,
    pub weekdays: Weekdays,
    pub start_min: u16,
    pub end_min: u16,
    pub label: String,
}

pub const MAX_BLOCK_LABEL_CHARS: usize = 60;

impl BlockDraft {
    /// Checks the rules and returns the draft with its label trimmed.
    pub fn validated(&self) -> Result<Self, RepoError> {
        use RepoError::Invalid;
        if self.weekdays.is_empty() {
            return Err(Invalid("a block needs at least one weekday"));
        }
        if self.end_min <= self.start_min {
            return Err(Invalid("a block must end after it starts"));
        }
        if self.end_min > 24 * 60 {
            return Err(Invalid("a block must end by midnight"));
        }
        let label = self.label.trim();
        if label.chars().count() > MAX_BLOCK_LABEL_CHARS {
            return Err(Invalid("a block label can be at most 60 characters"));
        }
        Ok(Self {
            label: label.to_owned(),
            ..self.clone()
        })
    }
}

impl Block {
    /// Whether the block is on at this local moment.
    pub fn is_on(&self, at: &Zoned) -> bool {
        let minute = at.hour() as u16 * 60 + at.minute() as u16;
        self.weekdays.contains(at.weekday()) && (self.start_min..self.end_min).contains(&minute)
    }

    /// `21:00-22:00`
    pub fn times_text(&self) -> String {
        format!(
            "{}-{}",
            clock_text(self.start_min),
            clock_text(self.end_min)
        )
    }
}

/// `21:00` from minutes after midnight.
pub fn clock_text(minutes: u16) -> String {
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

/// The block that is on at `now`; with overlaps, the one that started first.
pub fn active_block<'a>(blocks: &'a [Block], now: Timestamp, tz: &TimeZone) -> Option<&'a Block> {
    let local = now.to_zoned(tz.clone());
    blocks
        .iter()
        .filter(|b| b.is_on(&local))
        .min_by_key(|b| b.start_min)
}
