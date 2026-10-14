-- Correct review-only identity normalization. Catalog rows are never updated.
-- This is part of the initial coordinated review/admin release, before public
-- review submissions. Ordinary title/author keys retain their previous values.
CREATE OR REPLACE FUNCTION review_title_key(title TEXT, author TEXT, book_id TEXT)
RETURNS TEXT LANGUAGE SQL IMMUTABLE PARALLEL SAFE AS $$
 SELECT md5(json_build_array(
   lower(btrim(regexp_replace(title, '\s+', ' ', 'g'))),
   CASE WHEN btrim(regexp_replace(author, '\s+', ' ', 'g')) = ''
        THEN 'unknown-author:' || book_id
        ELSE lower(btrim(regexp_replace(author, '\s+', ' ', 'g'))) END
 )::text)
$$;

-- Preserve existing review/revision IDs while correcting stored identities.
-- A coordinated cutover excludes old application writers during this migration.
LOCK TABLE review_works, reviews, review_work_aggregates IN ACCESS EXCLUSIVE MODE;
CREATE TEMP TABLE review_identity_rekey ON COMMIT DROP AS
 SELECT r.id AS review_id,r.user_id,r.work_key AS old_key,
 review_title_key(b.title,COALESCE(a.name,''),b.id) AS new_key,
 b.title,COALESCE(a.name,'') AS author
 FROM reviews r JOIN books b ON b.id=r.book_id
 LEFT JOIN authors a ON a.id=b.primary_author_id;
DO $$ BEGIN
 IF EXISTS(SELECT 1 FROM review_identity_rekey GROUP BY new_key,user_id HAVING count(*)>1) THEN
  RAISE EXCEPTION 'Review normalization would merge multiple reviews by one account; resolve these review-only conflicts before retrying migration';
 END IF;
END $$;
INSERT INTO review_works(work_key,title,author)
 SELECT DISTINCT ON(new_key) new_key,title,author
 FROM review_identity_rekey ORDER BY new_key,review_id
 ON CONFLICT DO NOTHING;
UPDATE reviews r SET work_key=m.new_key FROM review_identity_rekey m
 WHERE r.id=m.review_id AND r.work_key<>m.new_key;
DELETE FROM review_work_aggregates a WHERE NOT EXISTS(SELECT 1 FROM reviews r WHERE r.work_key=a.work_key);
DELETE FROM review_works w WHERE NOT EXISTS(SELECT 1 FROM reviews r WHERE r.work_key=w.work_key);
INSERT INTO review_work_aggregates
 SELECT w.work_key,count(r.id),COALESCE(sum(r.rating),0),COALESCE(avg(r.rating),0),
 count(r.id) FILTER(WHERE r.rating=1),count(r.id) FILTER(WHERE r.rating=2),
 count(r.id) FILTER(WHERE r.rating=3),count(r.id) FILTER(WHERE r.rating=4),count(r.id) FILTER(WHERE r.rating=5)
 FROM review_works w LEFT JOIN reviews r ON r.work_key=w.work_key AND r.status='published' AND r.deleted_at IS NULL
 GROUP BY w.work_key
 ON CONFLICT(work_key) DO UPDATE SET published_count=EXCLUDED.published_count,rating_sum=EXCLUDED.rating_sum,
 average_rating=EXCLUDED.average_rating,star_1_count=EXCLUDED.star_1_count,star_2_count=EXCLUDED.star_2_count,
 star_3_count=EXCLUDED.star_3_count,star_4_count=EXCLUDED.star_4_count,star_5_count=EXCLUDED.star_5_count;
