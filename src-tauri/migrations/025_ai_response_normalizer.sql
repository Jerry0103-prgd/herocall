-- V1.1.2 compatibility patch: retain parse failures for local diagnosis without creating an
-- ai_reviews success row. Raw responses are redacted and bounded by the Rust service.
CREATE TABLE ai_review_failures (
  id INTEGER PRIMARY KEY,
  provider TEXT NOT NULL,
  model TEXT NOT NULL,
  security_id INTEGER NOT NULL REFERENCES securities(id) ON DELETE CASCADE,
  error_code TEXT NOT NULL CHECK (error_code = 'AI_REVIEW_PARSE_FAILED'),
  raw_response TEXT NOT NULL,
  error_message TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_ai_review_failures_security_created
  ON ai_review_failures(security_id, created_at DESC, id DESC);
