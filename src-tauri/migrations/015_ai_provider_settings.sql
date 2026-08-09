-- Only non-sensitive Provider preferences are persisted. API keys remain in the OS Keychain.
CREATE TABLE ai_provider_settings (
  provider TEXT PRIMARY KEY CHECK (provider IN ('DEEPSEEK', 'TENCENT_HUNYUAN', 'DOUBAO')),
  model TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 0 CHECK (enabled IN (0, 1)),
  priority INTEGER NOT NULL UNIQUE CHECK (priority BETWEEN 1 AND 3),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

INSERT INTO ai_provider_settings (provider, model, enabled, priority) VALUES
  ('DEEPSEEK', 'deepseek-chat', 1, 1),
  ('TENCENT_HUNYUAN', 'hunyuan-turbos-latest', 0, 2),
  ('DOUBAO', 'doubao-seed-1-6-250615', 0, 3);
