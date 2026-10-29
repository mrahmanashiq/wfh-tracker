-- Tracked time. Task sessions may run concurrently; contextual sessions remain
-- exclusive through the repository rules.

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
