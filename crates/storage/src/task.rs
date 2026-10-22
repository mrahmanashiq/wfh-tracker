use rusqlite::{OptionalExtension, Row, params};
use wt_core::repo::RepoResult;
use wt_core::{Id, Meta, RepoError, Task, TaskRepo};

use crate::{SqliteStore, backend, parse_id, parse_ts, require_changed};

const COLUMNS: &str = "id, parent_id, title, category_id, done_at, created_at, updated_at, deleted_at, break_interval_min";

/// Raw row, read inside the rusqlite closure and validated outside it.
struct TaskRow {
    id: String,
    parent_id: Option<String>,
    title: String,
    category_id: String,
    done_at: Option<String>,
    created_at: String,
    updated_at: String,
    deleted_at: Option<String>,
    break_interval_min: Option<u32>,
}

impl TaskRow {
    fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            parent_id: row.get(1)?,
            title: row.get(2)?,
            category_id: row.get(3)?,
            done_at: row.get(4)?,
            created_at: row.get(5)?,
            updated_at: row.get(6)?,
            deleted_at: row.get(7)?,
            break_interval_min: row.get(8)?,
        })
    }

    fn into_task(self) -> RepoResult<Task> {
        Ok(Task {
            meta: Meta {
                id: parse_id(&self.id)?,
                created_at: parse_ts(&self.created_at)?,
                updated_at: parse_ts(&self.updated_at)?,
                deleted_at: self.deleted_at.as_deref().map(parse_ts).transpose()?,
            },
            parent_id: self.parent_id.as_deref().map(parse_id).transpose()?,
            title: self.title,
            category_id: parse_id(&self.category_id)?,
            done_at: self.done_at.as_deref().map(parse_ts).transpose()?,
            break_interval_min: self.break_interval_min,
        })
    }
}

impl SqliteStore {
    fn query_tasks(&self, filter: &str, params: impl rusqlite::Params) -> RepoResult<Vec<Task>> {
        let mut stmt = self
            .conn
            .prepare(&format!(
                "SELECT {COLUMNS} FROM task WHERE deleted_at IS NULL {filter} ORDER BY id"
            ))
            .map_err(backend)?;
        let rows = stmt
            .query_map(params, TaskRow::read)
            .map_err(backend)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(backend)?;
        rows.into_iter().map(TaskRow::into_task).collect()
    }
}

impl TaskRepo for SqliteStore {
    fn list_live(&self) -> RepoResult<Vec<Task>> {
        self.query_tasks("", [])
    }

    fn get(&self, id: Id) -> RepoResult<Task> {
        self.conn
            .query_row(
                &format!("SELECT {COLUMNS} FROM task WHERE id = ?1 AND deleted_at IS NULL"),
                [id.to_string()],
                TaskRow::read,
            )
            .optional()
            .map_err(backend)?
            .ok_or(RepoError::NotFound(id))?
            .into_task()
    }

    fn subtasks(&self, id: Id) -> RepoResult<Vec<Task>> {
        self.query_tasks("AND parent_id = ?1", [id.to_string()])
    }

    fn insert(&self, parent_id: Option<Id>, title: &str, category_id: Id) -> RepoResult<Task> {
        let id = Id::new();
        self.conn
            .execute(
                "INSERT INTO task (id, parent_id, title, category_id, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                params![
                    id.to_string(),
                    parent_id.map(|p| p.to_string()),
                    title,
                    category_id.to_string(),
                    self.now()
                ],
            )
            .map_err(backend)?;
        self.get(id)
    }

    fn write_title(&self, id: Id, title: &str) -> RepoResult<()> {
        let changed = self
            .conn
            .execute(
                "UPDATE task SET title = ?2, updated_at = ?3 WHERE id = ?1 AND deleted_at IS NULL",
                params![id.to_string(), title, self.now()],
            )
            .map_err(backend)?;
        require_changed(id, changed)
    }

    fn write_category(&self, id: Id, category_id: Id) -> RepoResult<()> {
        let changed = self
            .conn
            .execute(
                "UPDATE task SET category_id = ?2, updated_at = ?3
                 WHERE (id = ?1 OR parent_id = ?1) AND deleted_at IS NULL",
                params![id.to_string(), category_id.to_string(), self.now()],
            )
            .map_err(backend)?;
        require_changed(id, changed)
    }

    fn write_done(&self, id: Id, done: bool) -> RepoResult<()> {
        let changed = self
            .conn
            .execute(
                "UPDATE task
                 SET done_at = CASE WHEN ?2 THEN COALESCE(done_at, ?3) ELSE NULL END,
                     updated_at = ?3
                 WHERE id = ?1 AND deleted_at IS NULL",
                params![id.to_string(), done, self.now()],
            )
            .map_err(backend)?;
        require_changed(id, changed)
    }

    fn write_break_interval(&self, id: Id, minutes: Option<u32>) -> RepoResult<()> {
        let changed = self
            .conn
            .execute(
                "UPDATE task SET break_interval_min = ?2, updated_at = ?3
                 WHERE id = ?1 AND deleted_at IS NULL",
                params![id.to_string(), minutes, self.now()],
            )
            .map_err(backend)?;
        require_changed(id, changed)
    }

    fn remove(&self, id: Id) -> RepoResult<()> {
        let changed = self
            .conn
            .execute(
                "UPDATE task SET deleted_at = ?2, updated_at = ?2
                 WHERE (id = ?1 OR parent_id = ?1) AND deleted_at IS NULL",
                params![id.to_string(), self.now()],
            )
            .map_err(backend)?;
        require_changed(id, changed)
    }
}
