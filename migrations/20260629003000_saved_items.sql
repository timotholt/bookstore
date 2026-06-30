PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS saved_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_key TEXT,
    user_id TEXT,
    copy_id INTEGER NOT NULL REFERENCES book_copies(id) ON DELETE CASCADE,
    quantity INTEGER NOT NULL DEFAULT 1 CHECK (quantity > 0),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK (session_key IS NOT NULL OR user_id IS NOT NULL)
);

CREATE INDEX IF NOT EXISTS idx_saved_items_session
ON saved_items(session_key, created_at)
WHERE session_key IS NOT NULL AND user_id IS NULL;

CREATE INDEX IF NOT EXISTS idx_saved_items_user ON saved_items(user_id, created_at);
CREATE INDEX IF NOT EXISTS idx_saved_items_copy ON saved_items(copy_id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_saved_items_unique_session_copy
ON saved_items(session_key, copy_id)
WHERE session_key IS NOT NULL AND user_id IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_saved_items_unique_user_copy
ON saved_items(user_id, copy_id)
WHERE user_id IS NOT NULL;
