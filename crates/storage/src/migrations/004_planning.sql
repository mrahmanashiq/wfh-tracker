-- Targets (time per category per week or month) and scheduled blocks.

CREATE TABLE target (
    id          TEXT PRIMARY KEY NOT NULL,
    category_id TEXT NOT NULL REFERENCES category (id),
    period      TEXT NOT NULL CHECK (period IN ('week', 'month')),
    minutes     INTEGER NOT NULL CHECK (minutes > 0),
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    deleted_at  TEXT
) STRICT;

-- One live target per category and period.
CREATE UNIQUE INDEX target_one_per_period ON target (category_id, period)
    WHERE deleted_at IS NULL;

CREATE TABLE block (
    id          TEXT PRIMARY KEY NOT NULL,
    category_id TEXT NOT NULL REFERENCES category (id),
    weekdays    INTEGER NOT NULL CHECK (weekdays BETWEEN 1 AND 127),
    start_min   INTEGER NOT NULL CHECK (start_min BETWEEN 0 AND 1439),
    end_min     INTEGER NOT NULL CHECK (end_min > start_min AND end_min <= 1440),
    label       TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    deleted_at  TEXT
) STRICT;
