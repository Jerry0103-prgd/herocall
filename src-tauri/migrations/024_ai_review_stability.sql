-- V1.1.2 stability patch: normalize completed records to an explicit successful state.
-- Historical rows are retained; only their status label is made queryable consistently.
UPDATE ai_reviews
SET request_status = 'SUCCESS'
WHERE request_status = 'COMPLETED';

-- The Review page reads the latest successful record for each followed security.
CREATE INDEX IF NOT EXISTS idx_ai_reviews_latest_success_per_security
  ON ai_reviews (review_id, security_id, created_at DESC, id DESC)
  WHERE request_status = 'SUCCESS' AND security_id IS NOT NULL;
