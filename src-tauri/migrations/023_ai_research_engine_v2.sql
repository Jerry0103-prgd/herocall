-- V1.1.2: auditable profiles and AI Research Report V2. Existing reviews and evidence remain readable.

CREATE TABLE security_profiles (
  security_id INTEGER PRIMARY KEY REFERENCES securities(id) ON DELETE CASCADE,
  profile_status TEXT NOT NULL CHECK (profile_status IN ('PENDING', 'VERIFIED', 'NO_DATA')) DEFAULT 'PENDING',
  company_description TEXT,
  industry TEXT,
  sector TEXT,
  tags_json TEXT NOT NULL DEFAULT '[]',
  business_model TEXT,
  historical_characteristics TEXT,
  source TEXT,
  source_url TEXT,
  fetched_at TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Existing followed securities get an explicit pending profile. No company facts are inferred
-- from an empty local securities record during this migration.
INSERT OR IGNORE INTO security_profiles (security_id, profile_status)
SELECT security_id, 'PENDING' FROM watchlist_items;

CREATE TABLE ai_research_reports (
  id INTEGER PRIMARY KEY,
  ai_review_id INTEGER NOT NULL UNIQUE REFERENCES ai_reviews(id) ON DELETE CASCADE,
  security_id INTEGER NOT NULL REFERENCES securities(id) ON DELETE CASCADE,
  core_drivers_json TEXT NOT NULL,
  market_thesis_json TEXT NOT NULL,
  bull_bear_analysis_json TEXT NOT NULL,
  future_catalysts_json TEXT NOT NULL,
  risk_factors_json TEXT NOT NULL,
  research_score_json TEXT NOT NULL,
  research_context_json TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX idx_ai_research_reports_security_created
  ON ai_research_reports(security_id, created_at DESC, id DESC);

ALTER TABLE ai_review_contexts ADD COLUMN research_context_json TEXT NOT NULL DEFAULT '{}';
