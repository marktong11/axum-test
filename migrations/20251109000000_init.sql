CREATE TABLE players (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE attempts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    player_id INTEGER NOT NULL REFERENCES players(id),
    question TEXT NOT NULL,
    correct INTEGER NOT NULL CHECK (correct IN (0, 1)),
    answered_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_attempts_player_id ON attempts(player_id);
