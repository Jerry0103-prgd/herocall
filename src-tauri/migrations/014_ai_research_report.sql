-- The seven-section user-facing report is separate from the existing audit columns.
-- FACTS, INFERENCES and RISKS remain preserved for all historical reviews.
ALTER TABLE ai_reviews ADD COLUMN report_json TEXT;
