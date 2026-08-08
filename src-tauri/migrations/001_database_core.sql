CREATE TABLE securities (
  id INTEGER PRIMARY KEY,
  symbol TEXT NOT NULL,
  name TEXT NOT NULL,
  market TEXT NOT NULL,
  instrument_type TEXT NOT NULL CHECK (instrument_type IN ('STOCK', 'ETF')),
  industry TEXT,
  concepts_json TEXT NOT NULL DEFAULT '[]',
  trading_rule TEXT NOT NULL DEFAULT 'UNCONFIRMED',
  is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE(symbol, market)
);

CREATE TABLE cash_accounts (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  currency TEXT NOT NULL DEFAULT 'CNY',
  available_to_buy TEXT NOT NULL DEFAULT '0',
  withdrawable_cash TEXT NOT NULL DEFAULT '0',
  pending_settlement TEXT NOT NULL DEFAULT '0',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE holdings (
  id INTEGER PRIMARY KEY,
  cash_account_id INTEGER NOT NULL REFERENCES cash_accounts(id) ON DELETE RESTRICT,
  security_id INTEGER NOT NULL REFERENCES securities(id) ON DELETE RESTRICT,
  quantity INTEGER NOT NULL DEFAULT 0 CHECK (quantity >= 0),
  available_quantity INTEGER NOT NULL DEFAULT 0 CHECK (available_quantity >= 0),
  average_cost TEXT NOT NULL DEFAULT '0',
  position_source TEXT NOT NULL DEFAULT 'MANUAL' CHECK (position_source IN ('MANUAL', 'INITIAL_POSITION', 'IMPORT')),
  as_of_date TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE(cash_account_id, security_id),
  CHECK (available_quantity <= quantity)
);

CREATE TABLE transactions (
  id INTEGER PRIMARY KEY,
  cash_account_id INTEGER NOT NULL REFERENCES cash_accounts(id) ON DELETE RESTRICT,
  security_id INTEGER NOT NULL REFERENCES securities(id) ON DELETE RESTRICT,
  side TEXT NOT NULL CHECK (side IN ('BUY', 'SELL', 'OPENING')),
  record_source TEXT NOT NULL DEFAULT 'MANUAL' CHECK (record_source IN ('MANUAL', 'IMPORT', 'INITIAL_POSITION')),
  trade_date TEXT NOT NULL,
  quantity INTEGER NOT NULL CHECK (quantity > 0),
  price TEXT NOT NULL,
  commission TEXT NOT NULL DEFAULT '0',
  stamp_duty TEXT NOT NULL DEFAULT '0',
  transfer_fee TEXT NOT NULL DEFAULT '0',
  other_fee TEXT NOT NULL DEFAULT '0',
  external_reference TEXT,
  import_batch_id TEXT,
  note TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_transactions_account_security_date
  ON transactions(cash_account_id, security_id, trade_date);

CREATE TABLE data_sources (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  source_type TEXT NOT NULL,
  priority INTEGER NOT NULL CHECK (priority BETWEEN 1 AND 3),
  base_url TEXT,
  enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
  status TEXT NOT NULL DEFAULT 'UNCONFIRMED',
  last_success_at TEXT,
  last_error TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE market_snapshots (
  id INTEGER PRIMARY KEY,
  data_source_id INTEGER NOT NULL REFERENCES data_sources(id) ON DELETE RESTRICT,
  snapshot_kind TEXT NOT NULL DEFAULT 'FULL',
  market_timestamp TEXT NOT NULL,
  fetched_at TEXT NOT NULL,
  delay_status TEXT NOT NULL CHECK (delay_status IN ('REALTIME', 'DELAYED', 'CLOSED', 'NO_DATA')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_market_snapshots_source_time
  ON market_snapshots(data_source_id, market_timestamp DESC);

CREATE TABLE market_quotes (
  id INTEGER PRIMARY KEY,
  market_snapshot_id INTEGER REFERENCES market_snapshots(id) ON DELETE SET NULL,
  security_id INTEGER NOT NULL REFERENCES securities(id) ON DELETE RESTRICT,
  data_source_id INTEGER NOT NULL REFERENCES data_sources(id) ON DELETE RESTRICT,
  current_price TEXT NOT NULL,
  change_pct TEXT,
  market_timestamp TEXT NOT NULL,
  fetched_at TEXT NOT NULL,
  delay_status TEXT NOT NULL CHECK (delay_status IN ('REALTIME', 'DELAYED', 'CLOSED', 'NO_DATA')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE(security_id, data_source_id, market_timestamp)
);

CREATE INDEX idx_market_quotes_security_time
  ON market_quotes(security_id, market_timestamp DESC);
