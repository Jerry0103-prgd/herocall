-- Phase 7-E2: preserve the exact public disclosures collected by one manual refresh.
-- No provider data is seeded; rows are created only after a user-triggered collection.

CREATE TABLE manual_refresh_news_articles (
  manual_refresh_run_id INTEGER NOT NULL REFERENCES manual_refresh_runs(id) ON DELETE CASCADE,
  news_article_id INTEGER NOT NULL REFERENCES news_articles(id) ON DELETE RESTRICT,
  PRIMARY KEY (manual_refresh_run_id, news_article_id)
);

CREATE TABLE manual_refresh_events (
  manual_refresh_run_id INTEGER NOT NULL REFERENCES manual_refresh_runs(id) ON DELETE CASCADE,
  event_id INTEGER NOT NULL REFERENCES events(id) ON DELETE RESTRICT,
  PRIMARY KEY (manual_refresh_run_id, event_id)
);

CREATE INDEX idx_manual_refresh_news_run ON manual_refresh_news_articles(manual_refresh_run_id);
CREATE INDEX idx_manual_refresh_events_run ON manual_refresh_events(manual_refresh_run_id);

-- SQLite cannot extend a CHECK constraint in place. Rebuild only the event table, retaining all
-- original IDs and fields, so existing calendar records and their source links survive intact.
CREATE TABLE events_v2 (
  id INTEGER PRIMARY KEY,
  event_type TEXT NOT NULL CHECK (event_type IN (
    'EARNINGS', 'DIVIDEND', 'EX_DIVIDEND', 'SHAREHOLDER_MEETING', 'MACRO_DATA', 'FED_MEETING',
    'COMPANY_ANNOUNCEMENT', 'MAJOR_MATTER'
  )),
  title TEXT NOT NULL,
  security_id INTEGER REFERENCES securities(id) ON DELETE SET NULL,
  event_time TEXT NOT NULL,
  timezone TEXT NOT NULL,
  source TEXT NOT NULL,
  source_url TEXT,
  status TEXT NOT NULL CHECK (status IN ('CONFIRMED', 'UNCONFIRMED', 'ARCHIVED')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

INSERT INTO events_v2 (
  id, event_type, title, security_id, event_time, timezone, source, source_url, status, created_at
)
SELECT id, event_type, title, security_id, event_time, timezone, source, source_url, status, created_at
FROM events;

DROP TABLE events;
ALTER TABLE events_v2 RENAME TO events;
CREATE INDEX idx_events_time ON events(event_time);
CREATE INDEX idx_events_status_time ON events(status, event_time);
CREATE INDEX idx_events_security_time ON events(security_id, event_time);
CREATE INDEX idx_events_source_url ON events(source, source_url) WHERE source_url IS NOT NULL;
