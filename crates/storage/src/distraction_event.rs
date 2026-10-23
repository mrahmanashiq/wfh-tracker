use jiff::Timestamp;
use rusqlite::{Row, params};
use wt_core::repo::RepoResult;
use wt_core::{DistractionEvent, DistractionEventRepo, Id, WatchContext};

use crate::{SqliteStore, backend, corrupt, format_ts, into_meta, parse_id, parse_ts, read_stamps};

const EVENT_COLUMNS: &str =
    "id, rule_id, context, started_at, ended_at, created_at, updated_at, deleted_at";

type EventRow = (
    String,
    String,
    String,
    String,
    Option<String>,
    [Option<String>; 3],
);

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

impl DistractionEventRepo for SqliteStore {
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
