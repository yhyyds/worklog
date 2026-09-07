CREATE TABLE IF NOT EXISTS daily_schedules (
  id TEXT PRIMARY KEY,
  work_date TEXT NOT NULL CHECK (work_date GLOB '????-??-??'),
  title TEXT NOT NULL CHECK (length(trim(title)) BETWEEN 1 AND 200),
  planned_start_minute INTEGER NOT NULL CHECK (planned_start_minute BETWEEN 0 AND 1439),
  planned_end_minute INTEGER NOT NULL CHECK (planned_end_minute BETWEEN 1 AND 1440),
  status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active','cancelled')),
  created_at_utc TEXT NOT NULL,
  updated_at_utc TEXT NOT NULL,
  row_version INTEGER NOT NULL DEFAULT 1 CHECK (row_version > 0),
  CHECK (planned_start_minute < planned_end_minute)
) STRICT;

CREATE INDEX IF NOT EXISTS ix_daily_schedules_date_time
ON daily_schedules(work_date, status, planned_start_minute, created_at_utc);

INSERT OR IGNORE INTO schema_migrations(version, name, applied_at_utc)
VALUES (10, 'daily_fixed_schedules', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
