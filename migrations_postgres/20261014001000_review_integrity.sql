-- Only review-owned tables are changed.
ALTER TABLE reviews ADD CONSTRAINT published_review_has_revision
 CHECK ((status='published') = (published_revision IS NOT NULL));
ALTER TABLE review_reports ADD CONSTRAINT reported_revision_belongs_to_review
 FOREIGN KEY(review_id,revision_id) REFERENCES review_revisions(review_id,id) ON DELETE CASCADE;
ALTER TABLE review_moderation_events ADD CONSTRAINT moderated_revision_belongs_to_review
 FOREIGN KEY(review_id,revision_id) REFERENCES review_revisions(review_id,id) ON DELETE CASCADE;
-- A voter's account deletion cascades vote rows without calling the service.
CREATE FUNCTION refresh_review_votes() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE review BIGINT;
BEGIN
 review := COALESCE(NEW.review_id,OLD.review_id);
 UPDATE reviews SET helpful_count=(SELECT count(*) FROM review_votes WHERE review_id=review AND vote=1),
 unhelpful_count=(SELECT count(*) FROM review_votes WHERE review_id=review AND vote=-1)
 WHERE id=review;
 RETURN NULL;
END $$;
CREATE TRIGGER review_votes_totals AFTER INSERT OR UPDATE OR DELETE ON review_votes
FOR EACH ROW EXECUTE FUNCTION refresh_review_votes();
