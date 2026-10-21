use rusqlite::params;
use wt_core::repo::RepoResult;
use wt_core::{Id, Settings, SettingsRepo};

use crate::{SqliteStore, backend};

impl SettingsRepo for SqliteStore {
    fn load(&self) -> RepoResult<Settings> {
        let mut stmt = self
            .conn
            .prepare("SELECT key, value FROM setting WHERE deleted_at IS NULL")
            .map_err(backend)?;
        let pairs = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(backend)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(backend)?;
        Ok(Settings::from_pairs(pairs)?)
    }

    fn save(&self, settings: &Settings) -> RepoResult<()> {
        settings.validate()?;
        let now = self.now();
        let tx = self.conn.unchecked_transaction().map_err(backend)?;
        for (key, value) in settings.to_pairs() {
            tx.execute(
                "INSERT INTO setting (id, key, value, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?4)
                 ON CONFLICT (key) DO UPDATE
                   SET value = excluded.value, updated_at = excluded.updated_at, deleted_at = NULL
                   WHERE setting.value <> excluded.value OR setting.deleted_at IS NOT NULL",
                params![Id::new().to_string(), key, value, now],
            )
            .map_err(backend)?;
        }
        tx.commit().map_err(backend)
    }
}
