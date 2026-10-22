use rusqlite::{OptionalExtension, Row, params};
use wt_core::repo::RepoResult;
use wt_core::{Block, BlockDraft, Id, Meta, Period, PlanRepo, RepoError, Target, Weekdays};

use crate::{SqliteStore, backend, parse_id, parse_ts, require_changed};

const TARGET_COLUMNS: &str = "id, category_id, period, minutes, created_at, updated_at, deleted_at";
const BLOCK_COLUMNS: &str =
    "id, category_id, weekdays, start_min, end_min, label, created_at, updated_at, deleted_at";

fn meta(row: &Row<'_>, from: usize) -> rusqlite::Result<[Option<String>; 3]> {
    Ok([row.get(from)?, row.get(from + 1)?, row.get(from + 2)?])
}

fn into_meta(id: &str, [created, updated, deleted]: [Option<String>; 3]) -> RepoResult<Meta> {
    let required =
        |v: Option<String>| v.ok_or_else(|| RepoError::Corrupt("missing timestamp".into()));
    Ok(Meta {
        id: parse_id(id)?,
        created_at: parse_ts(&required(created)?)?,
        updated_at: parse_ts(&required(updated)?)?,
        deleted_at: deleted.as_deref().map(parse_ts).transpose()?,
    })
}

type TargetRow = (String, String, String, u32, [Option<String>; 3]);
type BlockRow = (String, String, u8, u16, u16, String, [Option<String>; 3]);

fn read_target(row: &Row<'_>) -> rusqlite::Result<TargetRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        meta(row, 4)?,
    ))
}

fn read_block(row: &Row<'_>) -> rusqlite::Result<BlockRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        meta(row, 6)?,
    ))
}

fn into_target((id, category, period, minutes, stamps): TargetRow) -> RepoResult<Target> {
    Ok(Target {
        meta: into_meta(&id, stamps)?,
        category_id: parse_id(&category)?,
        period: period
            .parse::<Period>()
            .map_err(|e| RepoError::Corrupt(e.to_string()))?,
        minutes,
    })
}

fn into_block((id, category, days, start, end, label, stamps): BlockRow) -> RepoResult<Block> {
    Ok(Block {
        meta: into_meta(&id, stamps)?,
        category_id: parse_id(&category)?,
        weekdays: Weekdays::from_bits(days),
        start_min: start,
        end_min: end,
        label,
    })
}

impl SqliteStore {
    fn live_rows<R>(
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

    fn block(&self, id: &str) -> RepoResult<Block> {
        self.conn
            .query_row(
                &format!("SELECT {BLOCK_COLUMNS} FROM block WHERE id = ?1 AND deleted_at IS NULL"),
                [id],
                read_block,
            )
            .optional()
            .map_err(backend)?
            .map(into_block)
            .transpose()?
            .ok_or_else(|| RepoError::Corrupt(format!("block {id} vanished")))
    }

    fn soft_delete(&self, table: &str, id: Id) -> RepoResult<()> {
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

impl PlanRepo for SqliteStore {
    fn targets(&self) -> RepoResult<Vec<Target>> {
        self.live_rows(
            &format!("SELECT {TARGET_COLUMNS} FROM target WHERE deleted_at IS NULL ORDER BY id"),
            read_target,
        )?
        .into_iter()
        .map(into_target)
        .collect()
    }

    fn write_target(&self, category_id: Id, period: Period, minutes: u32) -> RepoResult<Target> {
        let now = self.now();
        let updated = self
            .conn
            .execute(
                "UPDATE target SET minutes = ?3, updated_at = ?4
                 WHERE category_id = ?1 AND period = ?2 AND deleted_at IS NULL",
                params![category_id.to_string(), period.as_str(), minutes, now],
            )
            .map_err(backend)?;
        if updated == 0 {
            self.conn
                .execute(
                    "INSERT INTO target (id, category_id, period, minutes, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                    params![
                        Id::new().to_string(),
                        category_id.to_string(),
                        period.as_str(),
                        minutes,
                        now
                    ],
                )
                .map_err(backend)?;
        }
        self.targets()?
            .into_iter()
            .find(|t| t.category_id == category_id && t.period == period)
            .ok_or_else(|| RepoError::Corrupt("target vanished".into()))
    }

    fn delete_target(&self, id: Id) -> RepoResult<()> {
        self.soft_delete("target", id)
    }

    fn blocks(&self) -> RepoResult<Vec<Block>> {
        self.live_rows(
            &format!(
                "SELECT {BLOCK_COLUMNS} FROM block WHERE deleted_at IS NULL ORDER BY start_min, id"
            ),
            read_block,
        )?
        .into_iter()
        .map(into_block)
        .collect()
    }

    fn insert_block(&self, d: &BlockDraft) -> RepoResult<Block> {
        let id = Id::new().to_string();
        self.conn
            .execute(
                "INSERT INTO block (id, category_id, weekdays, start_min, end_min, label,
                                    created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
                params![
                    id,
                    d.category_id.to_string(),
                    d.weekdays.bits(),
                    d.start_min,
                    d.end_min,
                    d.label,
                    self.now()
                ],
            )
            .map_err(backend)?;
        self.block(&id)
    }

    fn write_block(&self, id: Id, d: &BlockDraft) -> RepoResult<Block> {
        let changed = self
            .conn
            .execute(
                "UPDATE block SET category_id = ?2, weekdays = ?3, start_min = ?4, end_min = ?5,
                                  label = ?6, updated_at = ?7
                 WHERE id = ?1 AND deleted_at IS NULL",
                params![
                    id.to_string(),
                    d.category_id.to_string(),
                    d.weekdays.bits(),
                    d.start_min,
                    d.end_min,
                    d.label,
                    self.now()
                ],
            )
            .map_err(backend)?;
        require_changed(id, changed)?;
        self.block(&id.to_string())
    }

    fn delete_block(&self, id: Id) -> RepoResult<()> {
        self.soft_delete("block", id)
    }
}
