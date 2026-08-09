-- A followed security is a presentation and research preference, not a portfolio position.
-- Keep it separate from holdings so cancelling a follow never deletes positions or transactions.
CREATE TABLE watchlist_items (
  id INTEGER PRIMARY KEY,
  security_id INTEGER NOT NULL UNIQUE REFERENCES securities(id) ON DELETE RESTRICT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_watchlist_items_created_at ON watchlist_items(created_at DESC, id DESC);

-- Preserve every existing position or legacy zero-position watchlist entry as a follow.
INSERT OR IGNORE INTO watchlist_items (security_id, created_at, updated_at)
SELECT security_id, created_at, updated_at FROM holdings;
