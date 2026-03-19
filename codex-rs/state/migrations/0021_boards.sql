CREATE TABLE IF NOT EXISTS boards (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    last_selected_thread_id TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_boards_name ON boards(name);

CREATE TABLE IF NOT EXISTS board_sessions (
    board_id TEXT NOT NULL,
    thread_id TEXT NOT NULL,
    title_snapshot TEXT NOT NULL DEFAULT '',
    sort_order INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'unknown',
    last_seen_event_idx INTEGER,
    last_event_idx INTEGER,
    added_at INTEGER NOT NULL,
    removed_at INTEGER,
    PRIMARY KEY (board_id, thread_id),
    FOREIGN KEY (board_id) REFERENCES boards(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_board_sessions_board_order
    ON board_sessions(board_id, removed_at, sort_order, added_at);
