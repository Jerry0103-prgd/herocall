-- Tencent's legacy Hunyuan endpoint and TokenHub use different credentials. Keep only the
-- non-sensitive preference row in SQLite, migrate its enabled state, and leave the legacy
-- Keychain item untouched. The TokenHub key itself is stored under a distinct Keychain account.
ALTER TABLE ai_provider_settings RENAME TO ai_provider_settings_legacy_018;

CREATE TABLE ai_provider_settings (
  provider TEXT PRIMARY KEY CHECK (provider IN ('DEEPSEEK', 'TENCENT_TOKENHUB', 'DOUBAO')),
  model TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 0 CHECK (enabled IN (0, 1)),
  priority INTEGER NOT NULL UNIQUE CHECK (priority BETWEEN 1 AND 3),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

INSERT INTO ai_provider_settings (provider, model, enabled, priority, updated_at)
SELECT
  CASE provider
    WHEN 'TENCENT_HUNYUAN' THEN 'TENCENT_TOKENHUB'
    ELSE provider
  END,
  CASE provider
    WHEN 'TENCENT_HUNYUAN' THEN 'hunyuan-turbos-latest'
    ELSE model
  END,
  enabled,
  priority,
  updated_at
FROM ai_provider_settings_legacy_018;

DROP TABLE ai_provider_settings_legacy_018;
