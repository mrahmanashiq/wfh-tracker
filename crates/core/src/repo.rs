//! Persistence boundary. Storage crates implement these traits; the rest of
//! the app depends only on them, so SQLite can later be swapped for Neon.

use jiff::Timestamp;

use crate::{Category, Id, NewCategory, Rgb, Settings, SettingsError};

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
    #[error("storage backend: {0}")]
    Backend(String),
}

pub type RepoResult<T> = Result<T, RepoError>;

pub trait CategoryRepo {
    /// Live (not deleted) categories, oldest first.
    fn list(&self) -> RepoResult<Vec<Category>>;
    fn create(&self, new: NewCategory) -> RepoResult<Category>;
    fn update(&self, id: Id, name: &str, color: Rgb) -> RepoResult<Category>;
    /// Soft delete: the row stays so the deletion can sync.
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
