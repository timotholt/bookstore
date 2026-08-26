-- Merchandise (bookmark, replica letter, t-shirt) is sold new only -- there is
-- no used version. Flag these copies as new so they show the new-item pricing.
UPDATE book_copies SET is_new = true WHERE book_id IN ('m001', 'm002', 'm003');
