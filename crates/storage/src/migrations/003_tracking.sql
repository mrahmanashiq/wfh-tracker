-- Tracked time. At most one running session (ended_at IS NULL) at a time.

ALTER TABLE task ADD COLUMN break_interval_min INTEGER CHECK (break_interval_min > 0);

CREATE TABLE session (
    id          TEXT PRIMARY KEY NOT NULL,
    task_id     TEXT REFERENCES task (id),
    category_id TEXT NOT NULL REFERENCES category (id),
    kind        TEXT NOT NULL
                CHECK (kind IN ('office', 'meeting', 'learning', 'personal', 'break', 'custom')),
    started_at  TEXT NOT NULL,
    ended_at    TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    deleted_at  TEXT
) STRICT;

CREATE INDEX session_started ON session (started_at);

-- The database itself refuses a second running session.
CREATE UNIQUE INDEX session_one_running ON session (ended_at IS NULL)
    WHERE ended_at IS NULL AND deleted_at IS NULL;
