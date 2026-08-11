-- V1.1.1: source-attributed market intelligence. Legacy news/events remain intact.

CREATE TABLE intelligence_items (
  id INTEGER PRIMARY KEY,
  title TEXT NOT NULL,
  summary TEXT NOT NULL,
  source TEXT NOT NULL,
  source_type TEXT NOT NULL CHECK (source_type IN ('OFFICIAL', 'NEWS', 'INDUSTRY', 'COMMUNITY', 'SOCIAL', 'RUMOR')),
  source_url TEXT,
  published_at TEXT NOT NULL,
  fetched_at TEXT NOT NULL,
  credibility_level TEXT NOT NULL CHECK (credibility_level IN ('A', 'B', 'C', 'D', 'E')),
  intelligence_type TEXT NOT NULL,
  dedup_key TEXT NOT NULL,
  topic_key TEXT NOT NULL,
  importance_score INTEGER NOT NULL DEFAULT 0,
  heat_score INTEGER NOT NULL DEFAULT 0,
  status TEXT NOT NULL CHECK (status IN ('ACTIVE', 'UNVERIFIED', 'PARTIALLY_CONFIRMED')) DEFAULT 'ACTIVE',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE(dedup_key, source)
);

CREATE INDEX idx_intelligence_items_topic ON intelligence_items(topic_key, published_at DESC, id DESC);
CREATE INDEX idx_intelligence_items_priority ON intelligence_items(credibility_level, importance_score DESC, published_at DESC);

CREATE TABLE intelligence_security_relations (
  intelligence_item_id INTEGER NOT NULL REFERENCES intelligence_items(id) ON DELETE CASCADE,
  security_id INTEGER NOT NULL REFERENCES securities(id) ON DELETE CASCADE,
  PRIMARY KEY (intelligence_item_id, security_id)
);
CREATE INDEX idx_intelligence_security_relations_security ON intelligence_security_relations(security_id, intelligence_item_id);

CREATE TABLE manual_refresh_intelligence_items (
  manual_refresh_run_id INTEGER NOT NULL REFERENCES manual_refresh_runs(id) ON DELETE CASCADE,
  intelligence_item_id INTEGER NOT NULL REFERENCES intelligence_items(id) ON DELETE CASCADE,
  PRIMARY KEY (manual_refresh_run_id, intelligence_item_id)
);
CREATE INDEX idx_manual_refresh_intelligence_items_run ON manual_refresh_intelligence_items(manual_refresh_run_id, intelligence_item_id);

ALTER TABLE ai_review_contexts ADD COLUMN intelligence_json TEXT NOT NULL DEFAULT '{}';
