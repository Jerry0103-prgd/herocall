-- V1.0.9: Provider connection details are non-sensitive preferences. API keys remain only in
-- the system Keychain. Keep the legacy `model` field for historical compatibility while new
-- runtime code reads `endpoint` and `model_id`.
ALTER TABLE ai_provider_settings ADD COLUMN endpoint TEXT;
ALTER TABLE ai_provider_settings ADD COLUMN model_id TEXT;

UPDATE ai_provider_settings
SET endpoint = CASE provider
  WHEN 'DEEPSEEK' THEN 'https://api.deepseek.com/chat/completions'
  WHEN 'TENCENT_TOKENHUB' THEN 'https://tokenhub.tencentmaas.com/v1/chat/completions'
  WHEN 'DOUBAO' THEN 'https://ark.cn-beijing.volces.com/api/v3/chat/completions'
  ELSE NULL
END,
model_id = CASE provider
  WHEN 'DEEPSEEK' THEN 'deepseek-chat'
  WHEN 'TENCENT_TOKENHUB' THEN 'hy3'
  WHEN 'DOUBAO' THEN model
  ELSE model
END;

UPDATE ai_provider_settings
SET model = model_id
WHERE model_id IS NOT NULL;
