-- Phase 6-C: persisted, provider-attributed AI explanation of a structured daily review.

CREATE TABLE ai_reviews (
  id INTEGER PRIMARY KEY,
  review_id INTEGER NOT NULL REFERENCES daily_reviews(id) ON DELETE CASCADE,
  model TEXT NOT NULL,
  prompt_version TEXT NOT NULL,
  facts TEXT NOT NULL,
  inferences TEXT NOT NULL,
  risks TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_ai_reviews_review_created
  ON ai_reviews(review_id, created_at DESC, id DESC);
