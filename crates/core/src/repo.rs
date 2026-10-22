//! Persistence boundary. Storage crates implement these traits; the rest of
//! the app depends only on them, so SQLite can later be swapped for Neon.

use jiff::Timestamp;

use crate::task::clean_title;
use jiff::SignedDuration;

use crate::{
    Block, BlockDraft, Category, CategoryKind, Id, MeetingToggle, NewCategory, Period, Placement,
    Rgb, Session, Settings, SettingsError, Switch, Target, Task, TimerTarget,
};

/// Columns every record carries so a future sync can merge and propagate deletes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Meta {
    pub id: Id,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub deleted_at: Option<Timestamp>,
}

#[derive(Debug, thiserror::Error)]
pub enum RepoError {
    #[error("record {0} not found")]
    NotFound(Id),
    #[error("corrupt record: {0}")]
    Corrupt(String),
    #[error(transparent)]
    Settings(#[from] SettingsError),
    #[error("{0}")]
    Invalid(&'static str),
    #[error("still used by: {}", tasks.join(", "))]
    InUse { tasks: Vec<String> },
    #[error("storage backend: {0}")]
    Backend(String),
}

pub type RepoResult<T> = Result<T, RepoError>;

pub trait CategoryRepo {
    /// Live (not deleted) categories, oldest first.
    fn list(&self) -> RepoResult<Vec<Category>>;
    fn create(&self, new: NewCategory) -> RepoResult<Category>;
    fn update(&self, id: Id, name: &str, color: Rgb) -> RepoResult<Category>;
    /// Soft delete: the row stays so the deletion can sync. Refused with
    /// [`RepoError::InUse`] while live tasks still use the category.
    fn delete(&self, id: Id) -> RepoResult<()>;
    /// Inserts the default categories only into a database that has never
    /// had any. Returns whether it inserted.
    fn seed_defaults(&self) -> RepoResult<bool>;
}

pub trait SettingsRepo {
    /// Stored settings; anything never saved falls back to its default.
    fn load(&self) -> RepoResult<Settings>;
    fn save(&self, settings: &Settings) -> RepoResult<()>;
}

/// Tasks and their rules. Backends implement the storage primitives; the
/// provided methods (`add`, `rename`, `set_category`, `set_done`) hold the
/// domain rules, so every backend behaves the same. Callers should write
/// through the provided methods, not the `insert`/`write_*` primitives.
pub trait TaskRepo {
    /// Live tasks and sub-tasks, in creation order.
    fn list_live(&self) -> RepoResult<Vec<Task>>;
    /// A live task, or [`RepoError::NotFound`].
    fn get(&self, id: Id) -> RepoResult<Task>;
    fn subtasks(&self, id: Id) -> RepoResult<Vec<Task>>;
    fn insert(&self, parent_id: Option<Id>, title: &str, category_id: Id) -> RepoResult<Task>;
    fn write_title(&self, id: Id, title: &str) -> RepoResult<()>;
    /// Sets the category of a task and all its sub-tasks.
    fn write_category(&self, id: Id, category_id: Id) -> RepoResult<()>;
    /// Ticking keeps an existing `done_at`; unticking clears it.
    fn write_done(&self, id: Id, done: bool) -> RepoResult<()>;
    fn write_break_interval(&self, id: Id, minutes: Option<u32>) -> RepoResult<()>;
    /// Soft-deletes a task and its sub-tasks.
    fn remove(&self, id: Id) -> RepoResult<()>;

    /// Adds a task. A sub-task takes its parent's category, can't be nested
    /// further, and reopens a done parent.
    fn add(&self, title: &str, placement: Placement) -> RepoResult<Task> {
        let title = clean_title(title)?;
        match placement {
            Placement::TopLevel { category_id } => self.insert(None, &title, category_id),
            Placement::Under(parent_id) => {
                let parent = self.get(parent_id)?;
                if parent.parent_id.is_some() {
                    return Err(RepoError::Invalid("a sub-task can't have sub-tasks"));
                }
                let task = self.insert(Some(parent_id), &title, parent.category_id)?;
                if parent.is_done() {
                    self.write_done(parent_id, false)?;
                }
                Ok(task)
            }
        }
    }

    fn rename(&self, id: Id, title: &str) -> RepoResult<()> {
        self.write_title(id, &clean_title(title)?)
    }

    /// Changes a top-level task's category; its sub-tasks follow.
    fn set_category(&self, id: Id, category_id: Id) -> RepoResult<()> {
        if self.get(id)?.parent_id.is_some() {
            return Err(RepoError::Invalid(
                "a sub-task always has its parent's category",
            ));
        }
        self.write_category(id, category_id)
    }

    /// Sets a top-level task's own break interval (`None` = use the global one).
    fn set_break_interval(&self, id: Id, minutes: Option<u32>) -> RepoResult<()> {
        if self.get(id)?.parent_id.is_some() {
            return Err(RepoError::Invalid(
                "a sub-task uses its parent's break interval",
            ));
        }
        if minutes == Some(0) {
            return Err(RepoError::Invalid(
                "a break interval must be at least 1 minute",
            ));
        }
        self.write_break_interval(id, minutes)
    }

    /// The break interval a running task uses: its own, else its parent's.
    fn break_interval_for(&self, id: Id) -> RepoResult<Option<u32>> {
        let task = self.get(id)?;
        match (task.break_interval_min, task.parent_id) {
            (Some(own), _) => Ok(Some(own)),
            (None, Some(parent)) => Ok(self.get(parent)?.break_interval_min),
            (None, None) => Ok(None),
        }
    }

    /// Ticks or unticks a task. Ticking a parent's last open sub-task ticks
    /// the parent; unticking a sub-task reopens it. Ticking a parent by hand
    /// leaves its sub-tasks alone.
    fn set_done(&self, id: Id, done: bool) -> RepoResult<()> {
        self.write_done(id, done)?;
        let Some(parent_id) = self.get(id)?.parent_id else {
            return Ok(());
        };
        if done {
            self.finish_parent_if_complete(parent_id)
        } else {
            self.write_done(parent_id, false)
        }
    }

    /// Soft-deletes a task and its sub-tasks. Deleting a parent's last open
    /// sub-task leaves only done ones, so the parent is ticked.
    fn delete(&self, id: Id) -> RepoResult<()> {
        let parent_id = self.get(id)?.parent_id;
        self.remove(id)?;
        match parent_id {
            Some(parent_id) => self.finish_parent_if_complete(parent_id),
            None => Ok(()),
        }
    }

    /// Ticks a parent whose sub-tasks (at least one) are all done.
    fn finish_parent_if_complete(&self, parent_id: Id) -> RepoResult<()> {
        let subtasks = self.subtasks(parent_id)?;
        if !subtasks.is_empty() && subtasks.iter().all(Task::is_done) {
            self.write_done(parent_id, true)?;
        }
        Ok(())
    }
}

/// Tracked time. Backends implement the primitives; the provided methods hold
/// the rules (one running session, meetings, breaks, heartbeat recovery).
pub trait SessionRepo: TaskRepo + CategoryRepo {
    /// The running session, if any.
    fn running(&self) -> RepoResult<Option<Session>>;
    /// Live sessions still running or ended after `since`, oldest first.
    fn sessions_since(&self, since: jiff::Timestamp) -> RepoResult<Vec<Session>>;
    /// In one transaction: ends the running session now and starts a new one.
    fn switch_to(
        &self,
        task_id: Option<Id>,
        category_id: Id,
        kind: CategoryKind,
    ) -> RepoResult<Switch>;
    /// Ends the running session at `at` (default now); returns it.
    fn end_running(&self, at: Option<jiff::Timestamp>) -> RepoResult<Option<Session>>;
    /// Records that the app is alive now.
    fn beat(&self) -> RepoResult<()>;
    fn last_heartbeat(&self) -> RepoResult<Option<jiff::Timestamp>>;

    /// Starts a timer on a task (with its category) or a bare category.
    fn start(&self, target: TimerTarget) -> RepoResult<Session> {
        let (task_id, category_id) = match target {
            TimerTarget::Task(id) => (Some(id), self.get(id)?.category_id),
            TimerTarget::Category(id) => (None, id),
        };
        let kind = category(self, category_id)?.kind;
        Ok(self.switch_to(task_id, category_id, kind)?.started)
    }

    fn stop(&self) -> RepoResult<Option<Session>> {
        self.end_running(None)
    }

    /// Ends work and starts a break session.
    fn take_break(&self) -> RepoResult<Switch> {
        let category = first_of_kind(self, CategoryKind::Break)?;
        self.switch_to(None, category.meta.id, CategoryKind::Break)
    }

    /// Starts a meeting (pausing whatever ran) or ends the running one.
    /// Ending a meeting restarts the work it interrupted, at the same instant.
    fn toggle_meeting(&self) -> RepoResult<MeetingToggle> {
        if let Some(meeting) = self.running()?.filter(Session::is_meeting) {
            let paused = self.paused_by(&meeting)?;
            let meeting = self.end_running(None)?.unwrap_or(meeting);
            let resumed = match paused {
                Some(work) => match self.start(TimerTarget::resuming(&work)) {
                    Ok(session) => Some(session),
                    // The task was deleted during the meeting: nothing to restart.
                    Err(RepoError::NotFound(_)) => None,
                    Err(e) => return Err(e),
                },
                None => None,
            };
            return Ok(MeetingToggle::Ended { meeting, resumed });
        }
        let category = first_of_kind(self, CategoryKind::Meeting)?;
        let switch = self.switch_to(None, category.meta.id, CategoryKind::Meeting)?;
        Ok(MeetingToggle::Started {
            paused: switch.ended,
        })
    }

    /// The work session `meeting` interrupted: the one that ended at the very
    /// instant the meeting started. Stored data, so it survives a restart.
    fn paused_by(&self, meeting: &Session) -> RepoResult<Option<Session>> {
        let just_before = meeting
            .started_at
            .checked_sub(SignedDuration::from_millis(1))
            .unwrap_or(meeting.started_at);
        Ok(self
            .sessions_since(just_before)?
            .into_iter()
            .find(|s| s.ended_at == Some(meeting.started_at) && !s.is_break() && !s.is_meeting()))
    }

    /// If the app was gone longer than `max_gap` (closed, or the PC slept)
    /// while a timer ran, ends that timer at the last heartbeat and returns
    /// it. Always records a fresh heartbeat.
    fn recover(
        &self,
        now: jiff::Timestamp,
        max_gap: SignedDuration,
    ) -> RepoResult<Option<Session>> {
        let stopped = match (self.running()?, self.last_heartbeat()?) {
            (Some(running), Some(beat)) if now.duration_since(beat) > max_gap => {
                self.end_running(Some(beat.max(running.started_at)))?
            }
            _ => None,
        };
        self.beat()?;
        Ok(stopped)
    }
}

fn category(repo: &(impl CategoryRepo + ?Sized), id: Id) -> RepoResult<Category> {
    repo.list()?
        .into_iter()
        .find(|c| c.meta.id == id)
        .ok_or(RepoError::NotFound(id))
}

fn first_of_kind(repo: &(impl CategoryRepo + ?Sized), kind: CategoryKind) -> RepoResult<Category> {
    let missing = match kind {
        CategoryKind::Meeting => "add a category of kind Meeting first",
        CategoryKind::Break => "add a category of kind Break first",
        _ => "add a category of that kind first",
    };
    repo.list()?
        .into_iter()
        .find(|c| c.kind == kind)
        .ok_or(RepoError::Invalid(missing))
}

/// Targets, one per category and period. Backends implement the primitives;
/// the provided method validates.
pub trait TargetRepo {
    fn targets(&self) -> RepoResult<Vec<Target>>;
    /// Creates or replaces the live target for this category and period.
    fn write_target(&self, category_id: Id, period: Period, minutes: u32) -> RepoResult<Target>;
    fn delete_target(&self, id: Id) -> RepoResult<()>;

    /// Sets a category's target for a period, in whole hours (1 to the
    /// period's maximum). Setting it again replaces it.
    fn set_target(&self, category_id: Id, period: Period, hours: u32) -> RepoResult<Target> {
        if hours == 0 || hours > period.max_hours() {
            return Err(RepoError::Invalid(match period {
                Period::Week => "a weekly target must be 1–200 hours",
                Period::Month => "a monthly target must be 1–800 hours",
            }));
        }
        self.write_target(category_id, period, hours * 60)
    }
}

/// Scheduled blocks. Backends implement the primitives; the provided
/// methods validate.
pub trait BlockRepo {
    fn blocks(&self) -> RepoResult<Vec<Block>>;
    fn insert_block(&self, draft: &BlockDraft) -> RepoResult<Block>;
    fn write_block(&self, id: Id, draft: &BlockDraft) -> RepoResult<Block>;
    fn delete_block(&self, id: Id) -> RepoResult<()>;

    fn add_block(&self, draft: &BlockDraft) -> RepoResult<Block> {
        self.insert_block(&draft.validated()?)
    }

    fn update_block(&self, id: Id, draft: &BlockDraft) -> RepoResult<Block> {
        self.write_block(id, &draft.validated()?)
    }
}
