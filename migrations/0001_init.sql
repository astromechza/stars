CREATE TABLE users (
    id           INTEGER PRIMARY KEY,
    subject      TEXT NOT NULL UNIQUE,
    email        TEXT,
    display_name TEXT,
    created_at   TEXT NOT NULL
);

CREATE TABLE boards (
    id          INTEGER PRIMARY KEY,
    user_id     INTEGER NOT NULL REFERENCES users(id),
    name        TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    archived_at TEXT,
    sort_order  INTEGER NOT NULL
);
CREATE INDEX idx_boards_user ON boards(user_id);

CREATE TABLE toggles (
    board_id   INTEGER NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
    year       INTEGER NOT NULL,
    month      INTEGER NOT NULL,
    day        INTEGER NOT NULL,
    toggled_at TEXT NOT NULL,
    PRIMARY KEY (board_id, year, month, day)
);
