-- Phase 4: preserve all normalized market fields required for a traceable quote.
-- Existing quote records remain readable; new fields default only for historical rows.

ALTER TABLE market_quotes ADD COLUMN symbol TEXT NOT NULL DEFAULT '';
ALTER TABLE market_quotes ADD COLUMN security_name TEXT NOT NULL DEFAULT '';
ALTER TABLE market_quotes ADD COLUMN market TEXT NOT NULL DEFAULT 'UNKNOWN';
ALTER TABLE market_quotes ADD COLUMN previous_close TEXT;
ALTER TABLE market_quotes ADD COLUMN price_change TEXT;
ALTER TABLE market_quotes ADD COLUMN volume TEXT;
ALTER TABLE market_quotes ADD COLUMN volume_unit TEXT;
ALTER TABLE market_quotes ADD COLUMN turnover_amount TEXT;
ALTER TABLE market_quotes ADD COLUMN turnover_unit TEXT;
ALTER TABLE market_quotes ADD COLUMN source TEXT NOT NULL DEFAULT 'UNCONFIRMED';

CREATE INDEX idx_market_quotes_snapshot
  ON market_quotes(market_snapshot_id);
