-- Existing whole-portfolio reports remain readable with NULL security_id.
-- New V1.0.4 reports bind one review output to one followed security.
ALTER TABLE ai_reviews ADD COLUMN security_id INTEGER REFERENCES securities(id) ON DELETE SET NULL;
CREATE INDEX idx_ai_reviews_review_security_created
  ON ai_reviews(review_id, security_id, created_at DESC, id DESC);
