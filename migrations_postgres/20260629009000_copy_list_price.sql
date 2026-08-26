-- Original retail ("list") price per copy, used to show a struck-through
-- compare-at price next to the selling price on cards and the detail page.
ALTER TABLE book_copies ADD COLUMN IF NOT EXISTS list_price NUMERIC(10,2);

-- Seed a varied, deterministic list price above the selling price so the
-- markdown percentage differs per copy (instead of a flat, fake-looking rate).
-- Factor ranges 1.30x–1.66x by copy id, rounded to a .99 retail price.
UPDATE book_copies
SET list_price = ROUND(price * (1.30 + (id % 7) * 0.06), 0) - 0.01
WHERE list_price IS NULL;
