CREATE TABLE IF NOT EXISTS day_closures (
  work_date TEXT PRIMARY KEY CHECK (work_date GLOB '????-??-??'),
  next_work_date TEXT NOT NULL UNIQUE CHECK (next_work_date GLOB '????-??-??'),
  closed_at_utc TEXT NOT NULL,
  carried_count INTEGER NOT NULL CHECK (carried_count >= 0),
  skipped_count INTEGER NOT NULL CHECK (skipped_count >= 0),
  summary_json TEXT NOT NULL CHECK (json_valid(summary_json))
) STRICT;

INSERT OR IGNORE INTO schema_migrations(version, name, applied_at_utc)
VALUES (3, 'day_closing_and_carryover', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
