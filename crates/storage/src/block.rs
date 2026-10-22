use rusqlite::{OptionalExtension, Row, params};
use wt_core::repo::RepoResult;
use wt_core::{Block, BlockDraft, BlockRepo, Id, RepoError, Weekdays};

use crate::{SqliteStore, backend, into_meta, parse_id, read_stamps, require_changed};

const BLOCK_COLUMNS: &str =
    "id, category_id, weekdays, start_min, end_min, label, created_at, updated_at, deleted_at";

type BlockRow = (String, String, u8, u16, u16, String, [Option<String>; 3]);

fn read_block(row: &Row<'_>) -> rusqlite::Result<BlockRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        read_stamps(row, 6)?,
    ))
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
}

impl BlockRepo for SqliteStore {
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
