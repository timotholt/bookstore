-- Preserve real bibliographic metadata and distinguish demo prices from offers.
ALTER TABLE books ADD COLUMN publisher TEXT NOT NULL DEFAULT '';
ALTER TABLE books ADD COLUMN description TEXT NOT NULL DEFAULT '';
ALTER TABLE books ADD COLUMN cover_url TEXT NOT NULL DEFAULT '';
ALTER TABLE books ADD COLUMN metadata_source TEXT NOT NULL DEFAULT '';
ALTER TABLE books ADD COLUMN metadata_source_id TEXT NOT NULL DEFAULT '';
ALTER TABLE book_copies ADD COLUMN price_source TEXT NOT NULL DEFAULT '';
