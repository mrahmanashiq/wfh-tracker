//! SQLite-backed repositories for WFH Tracker.

mod category;
mod settings;

use std::path::Path;
use std::sync::Arc;

use jiff::Timestamp;
use rusqlite::Connection;
use rusqlite_migration::{M, Migrations};
use wt_core::{Clock, RepoError};

fn migrations() -> Migrations<'static> {
    Migrations::new(vec![M::up(include_str!("migrations/001_foundation.sql"))])
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

fn parse_ts(s: &str) -> Result<Timestamp, RepoError> {
    s.parse()
        .map_err(|_| RepoError::Corrupt(format!("bad timestamp `{s}`")))
}

#[cfg(test)]
mod tests {
    #[test]
    fn migrations_are_valid() {
        super::migrations().validate().unwrap();
    }
}
