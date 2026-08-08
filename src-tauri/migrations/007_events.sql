-- Phase 6-D: source-backed investment event calendar. No event records are seeded here.

CREATE TABLE events (
  id INTEGER PRIMARY KEY,
  event_type TEXT NOT NULL CHECK (event_type IN (
    'EARNINGS', 'DIVIDEND', 'EX_DIVIDEND', 'SHAREHOLDER_MEETING', 'MACRO_DATA', 'FED_MEETING'
  )),
  title TEXT NOT NULL,
  security_id INTEGER REFERENCES securities(id) ON DELETE SET NULL,
  event_time TEXT NOT NULL,
  timezone TEXT NOT NULL,
  source TEXT NOT NULL,
  source_url TEXT,
  status TEXT NOT NULL CHECK (status IN ('CONFIRMED', 'UNCONFIRMED', 'ARCHIVED')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_events_time ON events(event_time);
CREATE INDEX idx_events_status_time ON events(status, event_time);
CREATE INDEX idx_events_security_time ON events(security_id, event_time);
