-- V1.0.7: retain explicit security ownership for shareable news and event records.
-- Legacy single-security columns remain as the display-primary association for compatibility.

CREATE TABLE news_security_links (
  news_article_id INTEGER NOT NULL REFERENCES news_articles(id) ON DELETE CASCADE,
  security_id INTEGER NOT NULL REFERENCES securities(id) ON DELETE CASCADE,
  PRIMARY KEY (news_article_id, security_id)
);

CREATE INDEX idx_news_security_links_security
  ON news_security_links(security_id, news_article_id);

INSERT OR IGNORE INTO news_security_links (news_article_id, security_id)
SELECT id, related_security_id
FROM news_articles
WHERE related_security_id IS NOT NULL;

CREATE TABLE event_security_links (
  event_id INTEGER NOT NULL REFERENCES events(id) ON DELETE CASCADE,
  security_id INTEGER NOT NULL REFERENCES securities(id) ON DELETE CASCADE,
  PRIMARY KEY (event_id, security_id)
);

CREATE INDEX idx_event_security_links_security
  ON event_security_links(security_id, event_id);

INSERT OR IGNORE INTO event_security_links (event_id, security_id)
SELECT id, security_id
FROM events
WHERE security_id IS NOT NULL;
