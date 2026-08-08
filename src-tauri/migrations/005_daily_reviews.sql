-- Phase 6-B: locally generated, non-AI daily review snapshots.

CREATE TABLE daily_reviews (
  id INTEGER PRIMARY KEY,
  review_date TEXT NOT NULL UNIQUE,
  snapshot_id INTEGER REFERENCES market_snapshots(id) ON DELETE SET NULL,
  portfolio_summary TEXT NOT NULL,
  market_summary TEXT NOT NULL,
  holding_summary TEXT NOT NULL,
  risk_summary TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_daily_reviews_snapshot
  ON daily_reviews(snapshot_id);
