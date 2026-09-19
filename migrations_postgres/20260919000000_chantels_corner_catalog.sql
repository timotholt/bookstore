-- Preserve the applied seed migration checksum and only replace unchanged legacy demo copy.
UPDATE books
SET slug = 'chantels-corner-brass-bookmark',
    title = 'Chantel''s Corner Brass Bookmark',
    search_text = 'Chantel''s Corner Brass Bookmark accessory brass gift',
    updated_at = now()
WHERE id = 'm001'
  AND slug = 'davis-brass-bookmark'
  AND title = 'Davis''s Brass Bookmark';

UPDATE book_copies
SET notes = 'Polished brass with engraved Chantel''s Corner logo.',
    updated_at = now()
WHERE book_id = 'm001'
  AND notes = 'Polished brass with engraved Davis''s Books logo.';

UPDATE book_copies
SET staff_quote = replace(staff_quote, ' - Davis Team', ' - Chantel''s Corner team'),
    updated_at = now()
WHERE staff_quote LIKE '% - Davis Team';

UPDATE book_copies
SET seal_text = 'CC',
    updated_at = now()
WHERE id IN (1, 7, 12, 19)
  AND seal_text = 'DB';

UPDATE book_collections
SET description = 'Books highlighted by Chantel''s Corner staff.',
    updated_at = now()
WHERE slug = 'staff-picks'
  AND description = 'Books highlighted by Davis''s Books staff.';
