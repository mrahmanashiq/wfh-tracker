-- Every table carries id (UUID v7) + created_at/updated_at/deleted_at (RFC 3339, UTC)
-- so a future sync backend can merge rows and propagate deletions.

CREATE TABLE setting (
    id         TEXT PRIMARY KEY NOT NULL,
    key        TEXT NOT NULL UNIQUE,
    value      TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
) STRICT;

CREATE TABLE category (
    id         TEXT PRIMARY KEY NOT NULL,
    name       TEXT NOT NULL,
    kind       TEXT NOT NULL
               CHECK (kind IN ('office', 'meeting', 'learning', 'personal', 'break', 'custom')),
    color      TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
) STRICT;
