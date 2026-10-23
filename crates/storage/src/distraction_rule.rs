use rusqlite::{Row, params};
use wt_core::repo::RepoResult;
use wt_core::{DistractionRule, DistractionRuleRepo, Id, RuleDraft};

use crate::{SqliteStore, backend, corrupt, into_meta, read_stamps, require_changed};

const RULE_COLUMNS: &str = "id, pattern, match_on, enabled, created_at, updated_at, deleted_at";

type RuleRow = (String, String, String, bool, [Option<String>; 3]);

fn read_rule(row: &Row<'_>) -> rusqlite::Result<RuleRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        read_stamps(row, 4)?,
    ))
}

fn into_rule((id, pattern, match_on, enabled, stamps): RuleRow) -> RepoResult<DistractionRule> {
    Ok(DistractionRule {
        meta: into_meta(&id, stamps)?,
        pattern,
        match_on: match_on.parse().map_err(corrupt)?,
        enabled,
    })
}

impl DistractionRuleRepo for SqliteStore {
    fn rules(&self) -> RepoResult<Vec<DistractionRule>> {
        self.live_rows(
            &format!(
                "SELECT {RULE_COLUMNS} FROM distraction_rule WHERE deleted_at IS NULL
                 ORDER BY created_at, id"
            ),
            read_rule,
        )?
        .into_iter()
        .map(into_rule)
        .collect()
    }

    fn rules_ever(&self) -> RepoResult<u64> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM distraction_rule", [], |r| r.get(0))
            .map_err(backend)?;
        Ok(count.unsigned_abs())
    }

    fn insert_rule(&self, draft: &RuleDraft) -> RepoResult<DistractionRule> {
        let id = Id::new().to_string();
        self.conn
            .execute(
                "INSERT INTO distraction_rule (id, pattern, match_on, enabled, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 1, ?4, ?4)",
                params![id, draft.pattern, draft.match_on.as_str(), self.now()],
            )
            .map_err(backend)?;
        self.conn
            .query_row(
                &format!("SELECT {RULE_COLUMNS} FROM distraction_rule WHERE id = ?1"),
                [&id],
                read_rule,
            )
            .map_err(backend)
            .and_then(into_rule)
    }

    fn set_rule_enabled(&self, id: Id, enabled: bool) -> RepoResult<()> {
        let changed = self
            .conn
            .execute(
                "UPDATE distraction_rule SET enabled = ?2, updated_at = ?3
                 WHERE id = ?1 AND deleted_at IS NULL",
                params![id.to_string(), enabled, self.now()],
            )
            .map_err(backend)?;
        require_changed(id, changed)
    }

    fn delete_rule(&self, id: Id) -> RepoResult<()> {
        self.soft_delete("distraction_rule", id)
    }
}
