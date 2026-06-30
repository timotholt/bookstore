PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS reviews (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    book_id TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL,
    rating INTEGER NOT NULL CHECK (rating BETWEEN 1 AND 5),
    title TEXT NOT NULL DEFAULT '',
    body TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'published', 'rejected', 'removed')),
    verified_purchase INTEGER NOT NULL DEFAULT 0 CHECK (verified_purchase IN (0, 1)),
    verified_order_id TEXT,
    helpful_count INTEGER NOT NULL DEFAULT 0 CHECK (helpful_count >= 0),
    unhelpful_count INTEGER NOT NULL DEFAULT 0 CHECK (unhelpful_count >= 0),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    published_at TEXT,
    UNIQUE (book_id, user_id)
);

CREATE TABLE IF NOT EXISTS review_votes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    review_id INTEGER NOT NULL REFERENCES reviews(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL,
    vote INTEGER NOT NULL CHECK (vote IN (-1, 1)),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (review_id, user_id)
);

CREATE TABLE IF NOT EXISTS review_aggregates (
    book_id TEXT PRIMARY KEY REFERENCES books(id) ON DELETE CASCADE,
    published_count INTEGER NOT NULL DEFAULT 0 CHECK (published_count >= 0),
    rating_sum INTEGER NOT NULL DEFAULT 0 CHECK (rating_sum >= 0),
    average_rating REAL NOT NULL DEFAULT 0 CHECK (average_rating >= 0 AND average_rating <= 5),
    star_1_count INTEGER NOT NULL DEFAULT 0 CHECK (star_1_count >= 0),
    star_2_count INTEGER NOT NULL DEFAULT 0 CHECK (star_2_count >= 0),
    star_3_count INTEGER NOT NULL DEFAULT 0 CHECK (star_3_count >= 0),
    star_4_count INTEGER NOT NULL DEFAULT 0 CHECK (star_4_count >= 0),
    star_5_count INTEGER NOT NULL DEFAULT 0 CHECK (star_5_count >= 0),
    verified_count INTEGER NOT NULL DEFAULT 0 CHECK (verified_count >= 0),
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_reviews_book_status
ON reviews(book_id, status, published_at DESC);

CREATE INDEX IF NOT EXISTS idx_reviews_user
ON reviews(user_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_reviews_verified_order
ON reviews(verified_order_id)
WHERE verified_order_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_review_votes_review
ON review_votes(review_id);
