-- Phase 2-B: extend the 001 database without rewriting historical schema or data.

ALTER TABLE securities ADD COLUMN exchange TEXT NOT NULL DEFAULT 'UNKNOWN';
ALTER TABLE securities ADD COLUMN security_type TEXT NOT NULL DEFAULT 'STOCK'
  CHECK (security_type IN ('STOCK', 'ETF'));
ALTER TABLE securities ADD COLUMN trade_rule TEXT NOT NULL DEFAULT 'UNKNOWN'
  CHECK (trade_rule IN ('T_PLUS_1', 'T_PLUS_0', 'UNKNOWN'));

UPDATE securities
SET exchange = market
WHERE exchange = 'UNKNOWN';

UPDATE securities
SET security_type = instrument_type
WHERE security_type = 'STOCK' AND instrument_type IN ('STOCK', 'ETF');

UPDATE securities
SET trade_rule = CASE trading_rule
  WHEN 'T_PLUS_ONE' THEN 'T_PLUS_1'
  WHEN 'T_PLUS_ZERO' THEN 'T_PLUS_0'
  WHEN 'T_PLUS_1' THEN 'T_PLUS_1'
  WHEN 'T_PLUS_0' THEN 'T_PLUS_0'
  ELSE 'UNKNOWN'
END;

ALTER TABLE holdings ADD COLUMN cost_amount TEXT NOT NULL DEFAULT '0';

ALTER TABLE transactions ADD COLUMN status TEXT NOT NULL DEFAULT 'CONFIRMED'
  CHECK (status IN ('CONFIRMED', 'CANCELLED'));
ALTER TABLE transactions ADD COLUMN stamp_tax TEXT NOT NULL DEFAULT '0';
ALTER TABLE transactions ADD COLUMN minimum_commission TEXT NOT NULL DEFAULT '0';

UPDATE transactions
SET stamp_tax = stamp_duty
WHERE stamp_tax = '0' AND stamp_duty <> '0';

CREATE INDEX idx_transactions_status_date
  ON transactions(status, trade_date);

CREATE TABLE corporate_actions (
  id INTEGER PRIMARY KEY,
  security_id INTEGER NOT NULL REFERENCES securities(id) ON DELETE RESTRICT,
  action_type TEXT NOT NULL CHECK (action_type IN ('DIVIDEND', 'SPLIT', 'EX_RIGHT')),
  announcement_date TEXT,
  effective_date TEXT,
  data_source_id INTEGER REFERENCES data_sources(id) ON DELETE SET NULL,
  source_url TEXT,
  details_json TEXT NOT NULL DEFAULT '{}',
  status TEXT NOT NULL DEFAULT 'UNCONFIRMED',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_corporate_actions_security_date
  ON corporate_actions(security_id, effective_date);
