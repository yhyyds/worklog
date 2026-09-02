CREATE TABLE IF NOT EXISTS rest_sessions (
  id TEXT PRIMARY KEY,
  work_date TEXT NOT NULL CHECK (work_date GLOB '????-??-??'),
  focus_session_id TEXT NOT NULL UNIQUE REFERENCES focus_sessions(id) ON DELETE CASCADE,
  rest_kind TEXT NOT NULL CHECK (rest_kind IN ('short','long')),
  status TEXT NOT NULL CHECK (status IN ('running','paused','completed','skipped')),
  planned_seconds INTEGER NOT NULL CHECK (planned_seconds > 0),
  remaining_seconds INTEGER NOT NULL CHECK (remaining_seconds >= 0),
  target_end_at_utc TEXT,
  started_at_utc TEXT NOT NULL,
  ended_at_utc TEXT,
  active_guard INTEGER UNIQUE CHECK (active_guard IS NULL OR active_guard = 1),
  CHECK ((status IN ('running','paused') AND active_guard = 1) OR (status NOT IN ('running','paused') AND active_guard IS NULL))
) STRICT;

INSERT OR IGNORE INTO schema_migrations(version, name, applied_at_utc)
VALUES (5, 'focus_rest_lifecycle', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
