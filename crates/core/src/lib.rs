//! Domain types and rules for WFH Tracker.
//!
//! This crate does no I/O and knows nothing about the UI or the database.
//! Time comes in through [`Clock`]; persistence goes out through the
//! repository traits in [`repo`].

pub mod autostart;
pub mod category;
pub mod clock;
pub mod id;
pub mod notifier;
pub mod reminders;
pub mod repo;
pub mod session;
pub mod settings;
pub mod task;
pub mod updates;

pub use autostart::{Autostart, AutostartError};
pub use category::{Category, CategoryKind, NewCategory, ParseError, Rgb};
pub use clock::{Clock, FixedClock};
pub use id::Id;
pub use notifier::Notifier;
pub use reminders::{Notice, Reminders, Tick};
pub use repo::{CategoryRepo, Meta, RepoError, SessionRepo, SettingsRepo, TaskRepo};
pub use session::{
    MeetingToggle, OfficeStatus, Session, Switch, TimerTarget, office_status, office_time,
};
pub use settings::{Settings, SettingsError};
pub use task::{Placement, Task, TaskTree, today_view};
pub use updates::{AvailableUpdate, UpdateError, Updater};
