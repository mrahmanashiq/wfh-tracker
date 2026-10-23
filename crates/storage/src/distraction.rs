use jiff::Timestamp;
use rusqlite::{Row, params};
use wt_core::repo::RepoResult;
use wt_core::{
    DistractionEvent, DistractionRepo, DistractionRule, Id, RepoError, RuleDraft, WatchContext,
};

use crate::{
    SqliteStore, backend, format_ts, into_meta, parse_id, parse_ts, read_stamps, require_changed,
};

const RULE_COLUMNS: &str = "id, pattern, match_on, enabled, created_at, updated_at, deleted_at";
const EVENT_COLUMNS: &str =
    "id, rule_id, context, started_at, ended_at, created_at, updated_at, deleted_at";

type RuleRow = (String, String, String, bool, [Option<String>; 3]);
type EventRow = (
    String,
    String,
    String,
    String,
    Option<String>,
    [Option<String>; 3],
);

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

fn read_event(row: &Row<'_>) -> rusqlite::Result<EventRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        read_stamps(row, 5)?,
    ))
}

fn into_event((id, rule, context, start, end, stamps): EventRow) -> RepoResult<DistractionEvent> {
    Ok(DistractionEvent {
        meta: into_meta(&id, stamps)?,
        rule_id: parse_id(&rule)?,
        context: context.parse().map_err(corrupt)?,
        started_at: parse_ts(&start)?,
        ended_at: end.as_deref().map(parse_ts).transpose()?,
    })
}

fn corrupt(e: impl std::fmt::Display) -> RepoError {
    RepoError::Corrupt(e.to_string())
}

impl DistractionRepo for SqliteStore {
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

    fn insert_distraction(
        &self,
        rule_id: Id,
        context: WatchContext,
        started_at: Timestamp,
    ) -> RepoResult<DistractionEvent> {
        let id = Id::new().to_string();
        self.conn
            .execute(
                "INSERT INTO distraction_event (id, rule_id, context, started_at,
                                                created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                params![
                    id,
                    rule_id.to_string(),
                    context.as_str(),
                    format_ts(started_at),
                    self.now()
                ],
            )
            .map_err(backend)?;
        self.conn
            .query_row(
                &format!("SELECT {EVENT_COLUMNS} FROM distraction_event WHERE id = ?1"),
                [&id],
                read_event,
            )
            .map_err(backend)
            .and_then(into_event)
    }

    fn end_distractions(&self, at: Timestamp) -> RepoResult<()> {
        // Fixed-width timestamps compare as text, so MAX picks the later one.
        self.conn
            .execute(
                "UPDATE distraction_event SET ended_at = MAX(started_at, ?1), updated_at = ?2
                 WHERE ended_at IS NULL AND deleted_at IS NULL",
                params![format_ts(at), self.now()],
            )
            .map_err(backend)?;
        Ok(())
    }

    fn distractions_since(&self, since: Timestamp) -> RepoResult<Vec<DistractionEvent>> {
        let mut stmt = self
            .conn
            .prepare(&format!(
                "SELECT {EVENT_COLUMNS} FROM distraction_event
                 WHERE deleted_at IS NULL AND (ended_at IS NULL OR ended_at > ?1)
                 ORDER BY started_at, id"
            ))
            .map_err(backend)?;
        let rows = stmt
            .query_map([format_ts(since)], read_event)
            .map_err(backend)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(backend)?;
        rows.into_iter().map(into_event).collect()
    }
}
