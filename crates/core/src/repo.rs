//! Persistence boundary. Storage crates implement these traits; the rest of
//! the app depends only on them, so SQLite can later be swapped for Neon.

use jiff::Timestamp;

use crate::task::{MAX_DEPTH, clean_title};
use jiff::SignedDuration;

use crate::{
    Block, BlockDraft, Category, CategoryKind, DistractionEvent, DistractionRule, Id,
    MeetingToggle, NewCategory, NudgeCommand, Period, Placement, Rgb, RuleDraft, Session, Settings,
    SettingsError, Switch, Target, Task, TimerTarget, WatchContext,
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
    /// Sets the category of a task and every task below it.
    fn write_category(&self, id: Id, category_id: Id) -> RepoResult<()>;
    /// Ticking keeps an existing `done_at`; unticking clears it.
    fn write_done(&self, id: Id, done: bool) -> RepoResult<()>;
    fn write_break_interval(&self, id: Id, minutes: Option<u32>) -> RepoResult<()>;
    /// Soft-deletes a task and every task below it.
    fn remove(&self, id: Id) -> RepoResult<()>;

    /// The tasks above `id`, nearest first.
    fn ancestors(&self, id: Id) -> RepoResult<Vec<Task>> {
        let mut chain = Vec::new();
        let mut next = self.get(id)?.parent_id;
        while let Some(parent_id) = next {
            let parent = self.get(parent_id)?;
            next = parent.parent_id;
            chain.push(parent);
        }
        Ok(chain)
    }

    /// Adds a task. A sub-task takes its top-level task's category, nests at
    /// most [`MAX_DEPTH`] levels, and reopens any done task above it.
    fn add(&self, title: &str, placement: Placement) -> RepoResult<Task> {
        let title = clean_title(title)?;
        match placement {
            Placement::TopLevel { category_id } => self.insert(None, &title, category_id),
            Placement::Under(parent_id) => {
                let parent = self.get(parent_id)?;
                let above = self.ancestors(parent_id)?;
                if above.len() + 1 >= MAX_DEPTH {
                    return Err(RepoError::Invalid("tasks nest at most three levels"));
                }
                let task = self.insert(Some(parent_id), &title, parent.category_id)?;
                for done in std::iter::once(parent).chain(above).filter(Task::is_done) {
                    self.write_done(done.meta.id, false)?;
                }
                Ok(task)
            }
        }
    }

    fn rename(&self, id: Id, title: &str) -> RepoResult<()> {
        self.write_title(id, &clean_title(title)?)
    }

    /// Changes a top-level task's category; every task below follows.
    fn set_category(&self, id: Id, category_id: Id) -> RepoResult<()> {
        if self.get(id)?.parent_id.is_some() {
            return Err(RepoError::Invalid(
                "a sub-task always has its top-level task's category",
            ));
        }
        self.write_category(id, category_id)
    }

    /// Sets a top-level task's own break interval (`None` = use the global one).
    fn set_break_interval(&self, id: Id, minutes: Option<u32>) -> RepoResult<()> {
        if self.get(id)?.parent_id.is_some() {
            return Err(RepoError::Invalid(
                "a sub-task uses its top-level task's break interval",
            ));
        }
        if minutes == Some(0) {
            return Err(RepoError::Invalid(
                "a break interval must be at least 1 minute",
            ));
        }
        self.write_break_interval(id, minutes)
    }

    /// The break interval a running task uses: its own, else the nearest one
    /// above it.
    fn break_interval_for(&self, id: Id) -> RepoResult<Option<u32>> {
        let task = self.get(id)?;
        if task.break_interval_min.is_some() {
            return Ok(task.break_interval_min);
        }
        Ok(self
            .ancestors(id)?
            .into_iter()
            .find_map(|t| t.break_interval_min))
    }

    /// Ticks or unticks a task. Ticking the last open sub-task ticks its
    /// parent, and so on up; unticking reopens every done task above.
    /// Ticking a parent by hand leaves its sub-tasks alone.
    fn set_done(&self, id: Id, done: bool) -> RepoResult<()> {
        self.write_done(id, done)?;
        if done {
            return self.finish_chain_above(id);
        }
        for above in self.ancestors(id)?.into_iter().filter(Task::is_done) {
            self.write_done(above.meta.id, false)?;
        }
        Ok(())
    }

    /// Soft-deletes a task and everything below it. Deleting the last open
    /// sub-task can leave only done ones, which completes the tasks above.
    fn delete(&self, id: Id) -> RepoResult<()> {
        let parent_id = self.get(id)?.parent_id;
        self.remove(id)?;
        match parent_id {
            Some(parent_id) => {
                if self.finish_parent_if_complete(parent_id)? {
                    self.finish_chain_above(parent_id)?;
                }
                Ok(())
            }
            None => Ok(()),
        }
    }

    /// Walks up from `id`, ticking each parent whose sub-tasks are all done,
    /// and stops at the first that isn't complete.
    fn finish_chain_above(&self, id: Id) -> RepoResult<()> {
        for above in self.ancestors(id)? {
            if !self.finish_parent_if_complete(above.meta.id)? {
                break;
            }
        }
        Ok(())
    }

    /// Ticks a parent whose sub-tasks (at least one) are all done. Returns
    /// whether it is now complete.
    fn finish_parent_if_complete(&self, parent_id: Id) -> RepoResult<bool> {
        let subtasks = self.subtasks(parent_id)?;
        let complete = !subtasks.is_empty() && subtasks.iter().all(Task::is_done);
        if complete {
            self.write_done(parent_id, true)?;
        }
        Ok(complete)
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
                Period::Week => "a weekly target must be 1-200 hours",
                Period::Month => "a monthly target must be 1-800 hours",
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

/// Distraction rules. Backends implement the primitives; the provided
/// methods validate and seed. No window title ever reaches this trait.
pub trait DistractionRuleRepo {
    /// Live rules, oldest first (creation time, then id).
    fn rules(&self) -> RepoResult<Vec<DistractionRule>>;
    /// How many rules were ever stored, deleted ones included.
    fn rules_ever(&self) -> RepoResult<u64>;
    fn insert_rule(&self, draft: &RuleDraft) -> RepoResult<DistractionRule>;
    fn set_rule_enabled(&self, id: Id, enabled: bool) -> RepoResult<()>;
    /// Soft delete; the rule's events keep pointing at it.
    fn delete_rule(&self, id: Id) -> RepoResult<()>;

    fn add_rule(&self, draft: &RuleDraft) -> RepoResult<DistractionRule> {
        self.insert_rule(&draft.validated()?)
    }

    /// Inserts the default rules only into a database that has never had
    /// any. Returns whether it inserted.
    fn seed_default_rules(&self) -> RepoResult<bool> {
        if self.rules_ever()? > 0 {
            return Ok(false);
        }
        for draft in RuleDraft::defaults() {
            self.add_rule(&draft)?;
        }
        Ok(true)
    }
}

/// Distraction events. Backends implement the primitives; the provided
/// method carries out the nudge watcher's commands.
pub trait DistractionEventRepo {
    fn insert_distraction(
        &self,
        rule_id: Id,
        context: WatchContext,
        started_at: Timestamp,
    ) -> RepoResult<DistractionEvent>;
    /// Ends every open event at `at` (or at its start, if that is later).
    fn end_distractions(&self, at: Timestamp) -> RepoResult<()>;
    /// Live events still open or ended after `since`, oldest first.
    fn distractions_since(&self, since: Timestamp) -> RepoResult<Vec<DistractionEvent>>;

    /// Records what the nudge watcher decided. At most one event is open:
    /// starting one ends any other first.
    fn record(&self, command: &NudgeCommand) -> RepoResult<()> {
        match *command {
            NudgeCommand::StartEvent {
                rule,
                context,
                started_at,
            } => {
                self.end_distractions(started_at)?;
                self.insert_distraction(rule, context, started_at)
                    .map(|_| ())
            }
            NudgeCommand::EndEvent { at } => self.end_distractions(at),
            NudgeCommand::Nudge { .. } => Ok(()),
        }
    }
}
