-- Phase 7-D: user-triggered snapshots may include market indices as well as held securities.
-- Index identities are deliberately separate from the STOCK/ETF securities model.

CREATE TABLE market_index_quotes (
  id INTEGER PRIMARY KEY,
  market_snapshot_id INTEGER NOT NULL REFERENCES market_snapshots(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  symbol TEXT NOT NULL,
  current_price TEXT NOT NULL,
  change_pct TEXT,
  market_timestamp TEXT NOT NULL,
  fetched_at TEXT NOT NULL,
  delay_status TEXT NOT NULL CHECK (delay_status IN ('REALTIME', 'DELAYED', 'CLOSED', 'NO_DATA')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE(market_snapshot_id, symbol)
);

CREATE INDEX idx_market_index_quotes_symbol_time
  ON market_index_quotes(symbol, market_timestamp DESC);
