CREATE TABLE admins (
    id INTEGER PRIMARY KEY
) STRICT;

CREATE TABLE custom_commands (
    id      INTEGER PRIMARY KEY,
    source  TEXT NOT NULL,
    name    TEXT NOT NULL,
    content TEXT NOT NULL,
    UNIQUE(name, source)
) STRICT;
