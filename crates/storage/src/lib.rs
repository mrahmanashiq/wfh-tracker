//! SQLite-backed repositories for WFH Tracker.

mod block;
mod category;
mod distraction_event;
mod distraction_rule;
mod session;
mod settings;
mod target;
mod task;

use std::path::Path;
use std::sync::Arc;

use jiff::Timestamp;
use rusqlite::{Connection, Row, params};
use rusqlite_migration::{M, Migrations};
use wt_core::repo::RepoResult;
use wt_core::{BlockRepo, Clock, Id, Meta, RepoError, TargetRepo};

fn migrations() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(include_str!("migrations/001_foundation.sql")),
        M::up(include_str!("migrations/002_tasks.sql")),
        M::up(include_str!("migrations/003_tracking.sql")),
        M::up(include_str!("migrations/004_planning.sql")),
        M::up(include_str!("migrations/005_nudges.sql")),
        M::up(include_str!("migrations/006_concurrent_tasks.sql")),
    ])
}

/// One SQLite database implementing every repository trait.
pub struct SqliteStore {
    conn: Connection,
    clock: Arc<dyn Clock>,
}

impl SqliteStore {
    /// Opens (creating if needed) the database file and migrates it to the latest schema.
    pub fn open(path: &Path, clock: Arc<dyn Clock>) -> Result<Self, RepoError> {
        let conn = Connection::open(path).map_err(backend)?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(backend)?;
        Self::init(conn, clock)
    }

    pub fn open_in_memory(clock: Arc<dyn Clock>) -> Result<Self, RepoError> {
        Self::init(Connection::open_in_memory().map_err(backend)?, clock)
    }

    fn init(mut conn: Connection, clock: Arc<dyn Clock>) -> Result<Self, RepoError> {
        conn.pragma_update(None, "foreign_keys", "ON")
            .map_err(backend)?;
        migrations()
            .to_latest(&mut conn)
            .map_err(|e| RepoError::Backend(e.to_string()))?;
        Ok(Self { conn, clock })
    }

    fn now(&self) -> String {
        format_ts(self.clock.now())
    }
}

/// Fixed-width RFC 3339 (always 6 fractional digits, UTC), so comparing the
/// stored text gives the same order as comparing the times.
fn format_ts(ts: Timestamp) -> String {
    jiff::fmt::temporal::DateTimePrinter::new()
        .precision(Some(6))
        .timestamp_to_string(&ts)
}

fn backend(e: rusqlite::Error) -> RepoError {
    RepoError::Backend(e.to_string())
}

/// `NotFound` when an update touched no live row.
fn require_changed(id: Id, changed_rows: usize) -> Result<(), RepoError> {
    if changed_rows == 0 {
        Err(RepoError::NotFound(id))
    } else {
        Ok(())
    }
}

/// A stored value that doesn't parse.
fn corrupt(e: impl std::fmt::Display) -> RepoError {
    RepoError::Corrupt(e.to_string())
}

fn parse_id(s: &str) -> Result<Id, RepoError> {
    s.parse()
        .map_err(|_| RepoError::Corrupt(format!("bad id `{s}`")))
}

fn parse_ts(s: &str) -> Result<Timestamp, RepoError> {
    s.parse()
        .map_err(|_| RepoError::Corrupt(format!("bad timestamp `{s}`")))
}

// ---- Shared row helpers ------------------------------------------------------

/// The three sync timestamps of a row, starting at column `from`.
pub(crate) fn read_stamps(row: &Row<'_>, from: usize) -> rusqlite::Result<[Option<String>; 3]> {
    Ok([row.get(from)?, row.get(from + 1)?, row.get(from + 2)?])
}

pub(crate) fn into_meta(
    id: &str,
    [created, updated, deleted]: [Option<String>; 3],
) -> RepoResult<Meta> {
    let required =
        |v: Option<String>| v.ok_or_else(|| RepoError::Corrupt("missing timestamp".into()));
    Ok(Meta {
        id: parse_id(id)?,
        created_at: parse_ts(&required(created)?)?,
        updated_at: parse_ts(&required(updated)?)?,
        deleted_at: deleted.as_deref().map(parse_ts).transpose()?,
    })
}

impl SqliteStore {
    /// Live targets and blocks that use a category, named for an "in use" message.
    pub(crate) fn plan_users(&self, category_id: Id) -> RepoResult<Vec<String>> {
        let mut users: Vec<String> = TargetRepo::targets(self)?
            .into_iter()
            .filter(|t| t.category_id == category_id)
            .map(|t| format!("{}ly target", t.period))
            .collect();
        users.extend(
            BlockRepo::blocks(self)?
                .into_iter()
                .filter(|b| b.category_id == category_id)
                .map(|b| {
                    let name = if b.label.is_empty() {
                        b.times_text()
                    } else {
                        b.label
                    };
                    format!("{name} block")
                }),
        );
        Ok(users)
    }

    pub(crate) fn live_rows<R>(
        &self,
        sql: &str,
        read: fn(&Row<'_>) -> rusqlite::Result<R>,
    ) -> RepoResult<Vec<R>> {
        let mut stmt = self.conn.prepare(sql).map_err(backend)?;
        let rows = stmt
            .query_map([], read)
            .map_err(backend)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(backend)?;
        Ok(rows)
    }

    pub(crate) fn soft_delete(&self, table: &str, id: Id) -> RepoResult<()> {
        let changed = self
            .conn
            .execute(
                &format!(
                    "UPDATE {table} SET deleted_at = ?2, updated_at = ?2
                     WHERE id = ?1 AND deleted_at IS NULL"
                ),
                params![id.to_string(), self.now()],
            )
            .map_err(backend)?;
        require_changed(id, changed)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn migrations_are_valid() {
        super::migrations().validate().unwrap();
    }
}
