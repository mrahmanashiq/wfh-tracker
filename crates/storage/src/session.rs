use jiff::Timestamp;
use rusqlite::{OptionalExtension, Row, params};
use wt_core::repo::RepoResult;
use wt_core::{CategoryKind, Id, Meta, RepoError, Session, SessionRepo, Switch};

use crate::{SqliteStore, backend, format_ts, parse_id, parse_ts};

const COLUMNS: &str =
    "id, task_id, category_id, kind, started_at, ended_at, created_at, updated_at, deleted_at";

/// The heartbeat lives in the setting table; `Settings` ignores unknown keys.
const HEARTBEAT_KEY: &str = "heartbeat";

/// Raw row, read inside the rusqlite closure and validated outside it.
struct SessionRow {
    id: String,
    task_id: Option<String>,
    category_id: String,
    kind: String,
    started_at: String,
    ended_at: Option<String>,
    created_at: String,
    updated_at: String,
    deleted_at: Option<String>,
}

impl SessionRow {
    fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            task_id: row.get(1)?,
            category_id: row.get(2)?,
            kind: row.get(3)?,
            started_at: row.get(4)?,
            ended_at: row.get(5)?,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
            deleted_at: row.get(8)?,
        })
    }

    fn into_session(self) -> RepoResult<Session> {
        Ok(Session {
            meta: Meta {
                id: parse_id(&self.id)?,
                created_at: parse_ts(&self.created_at)?,
                updated_at: parse_ts(&self.updated_at)?,
                deleted_at: self.deleted_at.as_deref().map(parse_ts).transpose()?,
            },
            task_id: self.task_id.as_deref().map(parse_id).transpose()?,
            category_id: parse_id(&self.category_id)?,
            kind: self
                .kind
                .parse()
                .map_err(|e: wt_core::ParseError| RepoError::Corrupt(e.to_string()))?,
            started_at: parse_ts(&self.started_at)?,
            ended_at: self.ended_at.as_deref().map(parse_ts).transpose()?,
        })
    }
}

impl SqliteStore {
    fn session(&self, id: &str) -> RepoResult<Session> {
        self.conn
            .query_row(
                &format!("SELECT {COLUMNS} FROM session WHERE id = ?1"),
                [id],
                SessionRow::read,
            )
            .map_err(backend)?
            .into_session()
    }

    /// Ends the running session at `at`, returning its id if there was one.
    fn end_running_at(&self, at: &str) -> RepoResult<Option<String>> {
        let running: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM session WHERE ended_at IS NULL AND deleted_at IS NULL",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(backend)?;
        if let Some(id) = &running {
            self.conn
                .execute(
                    "UPDATE session SET ended_at = ?2, updated_at = ?3 WHERE id = ?1",
                    params![id, at, self.now()],
                )
                .map_err(backend)?;
        }
        Ok(running)
    }
}

impl SessionRepo for SqliteStore {
    fn running(&self) -> RepoResult<Option<Session>> {
        self.conn
            .query_row(
                &format!(
                    "SELECT {COLUMNS} FROM session WHERE ended_at IS NULL AND deleted_at IS NULL"
                ),
                [],
                SessionRow::read,
            )
            .optional()
            .map_err(backend)?
            .map(SessionRow::into_session)
            .transpose()
    }

    fn sessions_since(&self, since: Timestamp) -> RepoResult<Vec<Session>> {
        let mut stmt = self
            .conn
            .prepare(&format!(
                "SELECT {COLUMNS} FROM session
                 WHERE deleted_at IS NULL AND (ended_at IS NULL OR ended_at > ?1)
                 ORDER BY started_at, id"
            ))
            .map_err(backend)?;
        let rows = stmt
            .query_map([format_ts(since)], SessionRow::read)
            .map_err(backend)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(backend)?;
        rows.into_iter().map(SessionRow::into_session).collect()
    }

    fn switch_to(
        &self,
        task_id: Option<Id>,
        category_id: Id,
        kind: CategoryKind,
    ) -> RepoResult<Switch> {
        let now = self.now();
        let tx = self.conn.unchecked_transaction().map_err(backend)?;
        let ended = self.end_running_at(&now)?;
        let id = Id::new().to_string();
        tx.execute(
            "INSERT INTO session (id, task_id, category_id, kind, started_at, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?5)",
            params![
                id,
                task_id.map(|t| t.to_string()),
                category_id.to_string(),
                kind.as_str(),
                now
            ],
        )
        .map_err(backend)?;
        tx.commit().map_err(backend)?;
        Ok(Switch {
            ended: ended.map(|e| self.session(&e)).transpose()?,
            started: self.session(&id)?,
        })
    }

    fn end_running(&self, at: Option<Timestamp>) -> RepoResult<Option<Session>> {
        let at = at.map_or_else(|| self.now(), format_ts);
        self.end_running_at(&at)?
            .map(|id| self.session(&id))
            .transpose()
    }

    fn beat(&self) -> RepoResult<()> {
        let now = self.now();
        self.conn
            .execute(
                "INSERT INTO setting (id, key, value, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?3, ?3)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                params![Id::new().to_string(), HEARTBEAT_KEY, now],
            )
            .map_err(backend)?;
        Ok(())
    }

    fn last_heartbeat(&self) -> RepoResult<Option<Timestamp>> {
        self.conn
            .query_row(
                "SELECT value FROM setting WHERE key = ?1 AND deleted_at IS NULL",
                [HEARTBEAT_KEY],
                |r| r.get::<_, String>(0),
            )
            .optional()
            .map_err(backend)?
            .as_deref()
            .map(parse_ts)
            .transpose()
    }
}
