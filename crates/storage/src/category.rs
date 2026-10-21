use rusqlite::{OptionalExtension, Row, params};
use wt_core::repo::RepoResult;
use wt_core::{Category, CategoryRepo, Id, Meta, NewCategory, RepoError, Rgb};

use crate::{SqliteStore, backend, parse_ts};

const COLUMNS: &str = "id, name, kind, color, created_at, updated_at, deleted_at";

/// Raw row, read inside the rusqlite closure and validated outside it.
struct CategoryRow {
    id: String,
    name: String,
    kind: String,
    color: String,
    created_at: String,
    updated_at: String,
    deleted_at: Option<String>,
}

impl CategoryRow {
    fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            name: row.get(1)?,
            kind: row.get(2)?,
            color: row.get(3)?,
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
            deleted_at: row.get(6)?,
        })
    }

    fn into_category(self) -> RepoResult<Category> {
        Ok(Category {
            meta: Meta {
                id: self
                    .id
                    .parse()
                    .map_err(|_| RepoError::Corrupt(format!("bad id `{}`", self.id)))?,
                created_at: parse_ts(&self.created_at)?,
                updated_at: parse_ts(&self.updated_at)?,
                deleted_at: self.deleted_at.as_deref().map(parse_ts).transpose()?,
            },
            name: self.name,
            kind: self.kind.parse().map_err(corrupt)?,
            color: self.color.parse().map_err(corrupt)?,
        })
    }
}

impl SqliteStore {
    fn live_category(&self, id: Id) -> RepoResult<Category> {
        self.conn
            .query_row(
                &format!("SELECT {COLUMNS} FROM category WHERE id = ?1 AND deleted_at IS NULL"),
                [id.to_string()],
                CategoryRow::read,
            )
            .optional()
            .map_err(backend)?
            .ok_or(RepoError::NotFound(id))?
            .into_category()
    }
}

fn corrupt(e: impl std::fmt::Display) -> RepoError {
    RepoError::Corrupt(e.to_string())
}

/// `NotFound` when an update touched no live row.
fn require_changed(id: Id, changed_rows: usize) -> RepoResult<()> {
    if changed_rows == 0 {
        Err(RepoError::NotFound(id))
    } else {
        Ok(())
    }
}

impl CategoryRepo for SqliteStore {
    fn list(&self) -> RepoResult<Vec<Category>> {
        let mut stmt = self
            .conn
            .prepare(&format!(
                "SELECT {COLUMNS} FROM category WHERE deleted_at IS NULL ORDER BY created_at, id"
            ))
            .map_err(backend)?;
        let rows = stmt
            .query_map([], CategoryRow::read)
            .map_err(backend)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(backend)?;
        rows.into_iter().map(CategoryRow::into_category).collect()
    }

    fn create(&self, new: NewCategory) -> RepoResult<Category> {
        let id = Id::new();
        self.conn
            .execute(
                "INSERT INTO category (id, name, kind, color, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                params![
                    id.to_string(),
                    new.name,
                    new.kind.as_str(),
                    new.color.to_string(),
                    self.now()
                ],
            )
            .map_err(backend)?;
        self.live_category(id)
    }

    fn update(&self, id: Id, name: &str, color: Rgb) -> RepoResult<Category> {
        let changed = self
            .conn
            .execute(
                "UPDATE category SET name = ?2, color = ?3, updated_at = ?4
                 WHERE id = ?1 AND deleted_at IS NULL",
                params![id.to_string(), name, color.to_string(), self.now()],
            )
            .map_err(backend)?;
        require_changed(id, changed)?;
        self.live_category(id)
    }

    fn delete(&self, id: Id) -> RepoResult<()> {
        let changed = self
            .conn
            .execute(
                "UPDATE category SET deleted_at = ?2, updated_at = ?2
                 WHERE id = ?1 AND deleted_at IS NULL",
                params![id.to_string(), self.now()],
            )
            .map_err(backend)?;
        require_changed(id, changed)
    }

    fn seed_defaults(&self) -> RepoResult<bool> {
        let tx = self.conn.unchecked_transaction().map_err(backend)?;
        let rows_ever: i64 = tx
            .query_row("SELECT COUNT(*) FROM category", [], |r| r.get(0))
            .map_err(backend)?;
        if rows_ever > 0 {
            return Ok(false);
        }
        for new in NewCategory::defaults() {
            self.create(new)?;
        }
        tx.commit().map_err(backend)?;
        Ok(true)
    }
}
