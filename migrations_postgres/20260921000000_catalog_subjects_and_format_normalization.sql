-- Preserve source subjects separately from the storefront's controlled genres.
CREATE TABLE IF NOT EXISTS book_subjects (
    book_id TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE,
    subject TEXT NOT NULL,
    source TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (book_id, subject)
);

CREATE INDEX IF NOT EXISTS idx_book_subjects_subject ON book_subjects(subject);

INSERT INTO genres (slug, name)
VALUES
    ('imported-books', 'Books'),
    ('religion', 'Religion'),
    ('computers', 'Computers'),
    ('technology', 'Technology'),
    ('medicine', 'Medicine'),
    ('science', 'Science'),
    ('biography', 'Biography'),
    ('education', 'Education'),
    ('reference', 'Reference'),
    ('travel', 'Travel'),
    ('social-science', 'Social Science'),
    ('music', 'Music'),
    ('philosophy', 'Philosophy'),
    ('politics', 'Politics')
ON CONFLICT (slug) DO NOTHING;

-- Case-only differences are not meaningful format differences. Preserve
-- meaningful variants such as Trade Paperback and Mass Market Paperback.
UPDATE book_copies
SET format = 'Paperback'
WHERE lower(btrim(format)) = 'paperback';

UPDATE book_copies
SET format = 'Hardcover'
WHERE lower(btrim(format)) = 'hardcover';
