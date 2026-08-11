-- V1.1.0: immutable boundaries for AI research-agent preparation.
-- Existing manual refreshes and AI reviews remain readable; new research runs are additive.
CREATE TABLE research_runs (
  id INTEGER PRIMARY KEY,
  started_at TEXT NOT NULL,
  completed_at TEXT,
  indices_snapshot_id INTEGER REFERENCES market_snapshots(id) ON DELETE SET NULL,
  status TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE security_price_history (
  id INTEGER PRIMARY KEY,
  security_id INTEGER NOT NULL REFERENCES securities(id) ON DELETE CASCADE,
  trade_date TEXT NOT NULL,
  open_price TEXT NOT NULL,
  high_price TEXT NOT NULL,
  low_price TEXT NOT NULL,
  close_price TEXT NOT NULL,
  volume TEXT NOT NULL,
  amount TEXT NOT NULL,
  change_percent TEXT,
  source TEXT NOT NULL,
  market_timestamp TEXT NOT NULL,
  fetched_at TEXT NOT NULL,
  UNIQUE(security_id, trade_date, source)
);
CREATE INDEX idx_security_price_history_security_date
  ON security_price_history(security_id, trade_date DESC);

CREATE TABLE research_evidence (
  id INTEGER PRIMARY KEY,
  research_run_id INTEGER NOT NULL REFERENCES research_runs(id) ON DELETE CASCADE,
  security_id INTEGER NOT NULL REFERENCES securities(id) ON DELETE CASCADE,
  evidence_type TEXT NOT NULL,
  source TEXT,
  source_type TEXT,
  published_at TEXT,
  source_url TEXT,
  payload_json TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX idx_research_evidence_run_security_type
  ON research_evidence(research_run_id, security_id, evidence_type, id);

ALTER TABLE ai_review_contexts ADD COLUMN research_run_id INTEGER REFERENCES research_runs(id) ON DELETE SET NULL;
ALTER TABLE ai_reviews ADD COLUMN research_run_id INTEGER REFERENCES research_runs(id) ON DELETE SET NULL;
CREATE INDEX idx_ai_reviews_research_run_security
  ON ai_reviews(research_run_id, security_id, created_at DESC, id DESC);
