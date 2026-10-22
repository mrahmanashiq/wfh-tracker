//! Persistence boundary. Storage crates implement these traits; the rest of
//! the app depends only on them, so SQLite can later be swapped for Neon.

use jiff::Timestamp;

use crate::task::clean_title;
use crate::{Category, Id, NewCategory, Placement, Rgb, Settings, SettingsError, Task};

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
