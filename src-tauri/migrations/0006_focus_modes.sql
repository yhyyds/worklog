CREATE TABLE IF NOT EXISTS focus_session_modes (
  focus_session_id TEXT PRIMARY KEY REFERENCES focus_sessions(id) ON DELETE CASCADE,
  timer_mode TEXT NOT NULL CHECK (timer_mode IN ('countdown','count_up')),
  accumulated_seconds INTEGER NOT NULL DEFAULT 0 CHECK (accumulated_seconds >= 0),
  running_started_at_utc TEXT
) STRICT;

INSERT OR IGNORE INTO schema_migrations(version, name, applied_at_utc)
VALUES (6, 'focus_timer_modes', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
