-- Tasks with one level of sub-tasks. done_at is NULL while open.

CREATE TABLE task (
    id          TEXT PRIMARY KEY NOT NULL,
    parent_id   TEXT REFERENCES task (id),
    title       TEXT NOT NULL,
    category_id TEXT NOT NULL REFERENCES category (id),
    done_at     TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    deleted_at  TEXT
) STRICT;

CREATE INDEX task_parent ON task (parent_id);
CREATE INDEX task_category ON task (category_id);
