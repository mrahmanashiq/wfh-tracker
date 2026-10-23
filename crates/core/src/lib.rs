//! Domain types and rules for WFH Tracker.
//!
//! This crate does no I/O and knows nothing about the UI or the database.
//! Time comes in through [`Clock`]; persistence goes out through the
//! repository traits in [`repo`].

pub mod autostart;
pub mod block;
pub mod category;
pub mod clipboard;
pub mod clock;
pub mod distraction_event;
pub mod distraction_rule;
pub mod id;
pub mod notifier;
pub mod nudge;
pub mod reminders;
pub mod repo;
pub mod reports;
pub mod session;
pub mod settings;
pub mod target;
pub mod task;
pub mod updates;

pub use autostart::{Autostart, AutostartError};
pub use block::{Block, BlockDraft, Weekdays, active_block, clock_text};
pub use category::{Category, CategoryKind, NewCategory, ParseError, Rgb};
pub use clipboard::{Clipboard, ClipboardError};
pub use clock::{Clock, FixedClock};
pub use distraction_event::{DistractionEvent, WatchContext};
pub use distraction_rule::{DistractionRule, MatchOn, RuleDraft};
pub use id::Id;
pub use notifier::Notifier;
pub use nudge::{
    ForegroundWatcher, ForegroundWindow, NudgeCommand, NudgeWatcher, Sample, learning_block_on,
    nudge_title, watch_context,
};
pub use reminders::{Notice, Reminders, Tick};
pub use repo::{
    BlockRepo, CategoryRepo, DistractionEventRepo, DistractionRuleRepo, Meta, RepoError,
    SessionRepo, SettingsRepo, TargetRepo, TaskRepo,
};
pub use reports::{ReportInput, duration_text, my_time_report, office_report};
pub use session::{
    MeetingToggle, OfficeStatus, Session, Switch, TimerTarget, office_status, office_time,
    tracked_time,
};
pub use settings::{Settings, SettingsError};
pub use target::{Period, Target, period_range, target_progress};
pub use task::{Placement, Task, TaskTree, today_view};
pub use updates::{AvailableUpdate, UpdateError, Updater};
