CREATE TABLE command_usage (
    id    INTEGER PRIMARY KEY,
    year  INTEGER NOT NULL,
    month INTEGER NOT NULL,
    kind  TEXT NOT NULL,
    name  TEXT NOT NULL,
    count INTEGER NOT NULL,
    UNIQUE(year, month, kind, name)
) STRICT;