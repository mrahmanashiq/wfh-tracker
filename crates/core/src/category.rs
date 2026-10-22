use std::fmt;
use std::str::FromStr;

use crate::repo::Meta;

/// A stored value that doesn't name a known kind or colour.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct ParseError(String);

impl ParseError {
    pub fn new(message: String) -> Self {
        Self(message)
    }
}

/// What a category means to the rules. Office and Meeting count toward the
/// office clock; Break never counts toward anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CategoryKind {
    Office,
    Meeting,
    Learning,
    Personal,
    Break,
    Custom,
}

impl CategoryKind {
    pub const ALL: [Self; 6] = [
        Self::Office,
        Self::Meeting,
        Self::Learning,
        Self::Personal,
        Self::Break,
        Self::Custom,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Office => "office",
            Self::Meeting => "meeting",
            Self::Learning => "learning",
            Self::Personal => "personal",
            Self::Break => "break",
            Self::Custom => "custom",
        }
    }
}

impl FromStr for CategoryKind {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|k| k.as_str() == s)
            .ok_or_else(|| ParseError(format!("unknown category kind `{s}`")))
    }
}

/// A display colour, stored as `#rrggbb`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl fmt::Display for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

impl FromStr for Rgb {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let hex = s
            .strip_prefix('#')
            .filter(|h| h.len() == 6 && h.is_ascii())
            .ok_or_else(|| ParseError(format!("colour `{s}` is not #rrggbb")))?;
        let byte = |i: usize| {
            u8::from_str_radix(&hex[i..i + 2], 16)
                .map_err(|_| ParseError(format!("colour `{s}` is not #rrggbb")))
        };
        Ok(Self(byte(0)?, byte(2)?, byte(4)?))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Category {
    pub meta: Meta,
    pub name: String,
    pub kind: CategoryKind,
    pub color: Rgb,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewCategory {
    pub name: String,
    pub kind: CategoryKind,
    pub color: Rgb,
}

impl NewCategory {
    pub fn new(name: &str, kind: CategoryKind, color: Rgb) -> Self {
        Self {
            name: name.to_owned(),
            kind,
            color,
        }
    }

    /// The categories a fresh install starts with.
    pub fn defaults() -> Vec<Self> {
        use CategoryKind::*;
        vec![
            Self::new("Office", Office, Rgb(0x3b, 0x82, 0xf6)),
            Self::new("Meeting", Meeting, Rgb(0x8b, 0x5c, 0xf6)),
            Self::new("Learning", Learning, Rgb(0x10, 0xb9, 0x81)),
            Self::new("Family", Personal, Rgb(0xf5, 0x9e, 0x0b)),
            Self::new("Personal", Personal, Rgb(0xec, 0x48, 0x99)),
            Self::new("Social media", Personal, Rgb(0xef, 0x44, 0x44)),
            Self::new("Break", Break, Rgb(0x64, 0x74, 0x8b)),
        ]
    }
}
