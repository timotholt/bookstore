PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS carts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_key TEXT,
    user_id TEXT,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'merged', 'checked_out', 'abandoned')),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK (session_key IS NOT NULL OR user_id IS NOT NULL)
);

CREATE TABLE IF NOT EXISTS cart_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    cart_id INTEGER NOT NULL REFERENCES carts(id) ON DELETE CASCADE,
    copy_id INTEGER NOT NULL REFERENCES book_copies(id) ON DELETE CASCADE,
    quantity INTEGER NOT NULL CHECK (quantity > 0),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (cart_id, copy_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_carts_active_session
ON carts(session_key)
WHERE session_key IS NOT NULL AND user_id IS NULL AND status = 'active';

CREATE INDEX IF NOT EXISTS idx_carts_active_user ON carts(user_id, status);
CREATE INDEX IF NOT EXISTS idx_cart_items_cart ON cart_items(cart_id);
CREATE INDEX IF NOT EXISTS idx_cart_items_copy ON cart_items(copy_id);
