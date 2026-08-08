CREATE TABLE manual_refresh_runs (
  id INTEGER PRIMARY KEY,
  started_at TEXT NOT NULL,
  completed_at TEXT NOT NULL,
  holdings_snapshot_id INTEGER REFERENCES market_snapshots(id) ON DELETE SET NULL,
  indices_snapshot_id INTEGER REFERENCES market_snapshots(id) ON DELETE SET NULL,
  portfolio_json TEXT NOT NULL,
  news_status TEXT NOT NULL CHECK (news_status IN ('NO_DATA')),
  events_status TEXT NOT NULL CHECK (events_status IN ('NO_DATA')),
  status TEXT NOT NULL CHECK (status IN ('COMPLETED', 'NO_DATA')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_manual_refresh_runs_completed_at ON manual_refresh_runs(completed_at DESC);

CREATE TABLE ai_review_contexts (
  id INTEGER PRIMARY KEY,
  review_id INTEGER NOT NULL REFERENCES daily_reviews(id) ON DELETE CASCADE,
  manual_refresh_run_id INTEGER NOT NULL REFERENCES manual_refresh_runs(id) ON DELETE RESTRICT,
  portfolio_json TEXT NOT NULL,
  market_json TEXT NOT NULL,
  news_json TEXT NOT NULL,
  events_json TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

ALTER TABLE ai_reviews ADD COLUMN context_id INTEGER REFERENCES ai_review_contexts(id) ON DELETE SET NULL;
ALTER TABLE ai_reviews ADD COLUMN provider TEXT NOT NULL DEFAULT 'UNCONFIRMED';
ALTER TABLE ai_reviews ADD COLUMN request_status TEXT NOT NULL DEFAULT 'COMPLETED';
ALTER TABLE ai_reviews ADD COLUMN error_code TEXT;
