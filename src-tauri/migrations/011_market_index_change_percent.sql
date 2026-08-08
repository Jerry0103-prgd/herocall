-- Preserve the original provider field (`change_pct`) while adding the canonical field consumed
-- by the AI snapshot context. The migration runner applies this file only once in a transaction.
ALTER TABLE market_index_quotes ADD COLUMN change_percent TEXT;

-- Backfill every existing persisted index quote without changing source values or deleting data.
UPDATE market_index_quotes
SET change_percent = change_pct
WHERE change_percent IS NULL;
