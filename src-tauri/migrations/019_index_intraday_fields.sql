-- V1.0.8: retain source-backed intraday fields needed by the market overview.
-- All fields are nullable so existing snapshots remain readable without fabricated backfills.
ALTER TABLE market_index_quotes ADD COLUMN open_price TEXT;
ALTER TABLE market_index_quotes ADD COLUMN high_price TEXT;
ALTER TABLE market_index_quotes ADD COLUMN low_price TEXT;
ALTER TABLE market_index_quotes ADD COLUMN turnover_amount TEXT;
