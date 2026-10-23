-- Distraction rules and the distraction events recorded for them.
-- Only rule ids and times are stored, never window titles.

CREATE TABLE distraction_rule (
    id         TEXT PRIMARY KEY NOT NULL,
    pattern    TEXT NOT NULL CHECK (length(pattern) BETWEEN 1 AND 60),
    match_on   TEXT NOT NULL CHECK (match_on IN ('title', 'process')),
    enabled    INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
) STRICT;

CREATE TABLE distraction_event (
    id         TEXT PRIMARY KEY NOT NULL,
    rule_id    TEXT NOT NULL REFERENCES distraction_rule (id),
    context    TEXT NOT NULL CHECK (context IN ('learning', 'office')),
    started_at TEXT NOT NULL,
    ended_at   TEXT CHECK (ended_at IS NULL OR ended_at >= started_at),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
) STRICT;

CREATE INDEX distraction_event_started ON distraction_event (started_at);

-- At most one distraction is open at a time.
CREATE UNIQUE INDEX distraction_event_one_open ON distraction_event (ended_at IS NULL)
    WHERE ended_at IS NULL AND deleted_at IS NULL;
