//! Domain types and rules for WFH Tracker.
//!
//! This crate does no I/O and knows nothing about the UI or the database.
//! Time comes in through [`Clock`]; persistence goes out through the
//! repository traits in [`repo`].

pub mod autostart;
pub mod category;
pub mod clock;
pub mod id;
pub mod repo;
pub mod settings;

pub use autostart::{Autostart, AutostartError};
pub use category::{Category, CategoryKind, NewCategory, ParseError, Rgb};
pub use clock::{Clock, FixedClock};
pub use id::Id;
pub use repo::{CategoryRepo, Meta, RepoError, SettingsRepo};
pub use settings::{Settings, SettingsError};
