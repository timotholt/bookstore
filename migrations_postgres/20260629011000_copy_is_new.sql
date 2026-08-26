-- Whether a copy is brand-new (from the manufacturer) or pre-owned (used).
-- This is independent of `condition` (quality grade): a used copy can be in
-- like-new condition yet still be used. The new/used flag drives the discount
-- UI (new items show a struck-through list price + percent off).
ALTER TABLE book_copies ADD COLUMN IF NOT EXISTS is_new BOOLEAN NOT NULL DEFAULT false;

-- Demo: stock a handful of titles as brand-new alongside the used inventory.
UPDATE book_copies SET is_new = true WHERE id IN (3, 5, 9, 13, 17, 25);

-- Give those new copies a manufacturer-grade condition (deterministic, so the
-- demo is coherent whether or not the earlier condition demo applied).
UPDATE book_copies SET condition = 'New (Sealed)' WHERE id IN (3, 9, 17);
UPDATE book_copies SET condition = 'New (Open)' WHERE id IN (5, 13, 25);
