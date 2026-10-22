use rusqlite::{Row, params};
use wt_core::repo::RepoResult;
use wt_core::{Id, Period, RepoError, Target, TargetRepo};

use crate::{SqliteStore, backend, into_meta, parse_id, read_stamps};

const TARGET_COLUMNS: &str = "id, category_id, period, minutes, created_at, updated_at, deleted_at";

type TargetRow = (String, String, String, u32, [Option<String>; 3]);

fn read_target(row: &Row<'_>) -> rusqlite::Result<TargetRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        read_stamps(row, 4)?,
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

impl TargetRepo for SqliteStore {
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
}
