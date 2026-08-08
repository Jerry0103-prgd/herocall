-- Phase 6-A: traceable local news storage. No provider data is seeded by this migration.

CREATE TABLE news_articles (
  id INTEGER PRIMARY KEY,
  title TEXT NOT NULL,
  source TEXT NOT NULL,
  source_type TEXT NOT NULL CHECK (source_type IN ('OFFICIAL', 'MEDIA', 'COMMUNITY')),
  published_at TEXT NOT NULL,
  fetch_time TEXT NOT NULL,
  summary TEXT NOT NULL,
  url TEXT NOT NULL UNIQUE,
  related_security_id INTEGER REFERENCES securities(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_news_articles_published_at
  ON news_articles(published_at DESC, id DESC);

CREATE INDEX idx_news_articles_related_security
  ON news_articles(related_security_id, published_at DESC);
