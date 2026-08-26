-- Demo: stock a handful of titles as brand-new (alongside the used inventory)
-- so the new-item discount UI (strike-through list price + percent off) is
-- visible. New items show a markdown; used items just show "from $X".
-- 'New (Sealed)' / 'New (Open)' are the schema's allowed new-condition values.
UPDATE book_copies SET condition = 'New (Sealed)' WHERE id IN (3, 9, 17);
UPDATE book_copies SET condition = 'New (Open)' WHERE id IN (5, 13, 25);
