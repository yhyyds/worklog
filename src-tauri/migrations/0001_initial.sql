PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
PRAGMA synchronous = FULL;
PRAGMA busy_timeout = 5000;

CREATE TABLE IF NOT EXISTS schema_migrations (
  version INTEGER PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  applied_at_utc TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS app_settings (
  key TEXT PRIMARY KEY,
  value_json TEXT NOT NULL CHECK (json_valid(value_json)),
  updated_at_utc TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS tasks (
  id TEXT PRIMARY KEY,
  parent_task_id TEXT REFERENCES tasks(id) ON DELETE RESTRICT,
  title TEXT NOT NULL CHECK (length(trim(title)) > 0),
  status TEXT NOT NULL DEFAULT 'not_started' CHECK (status IN ('not_started','in_progress','waiting','blocked','completed','deferred','cancelled')),
  description_md TEXT,
  created_at_utc TEXT NOT NULL,
  updated_at_utc TEXT NOT NULL,
  completed_at_utc TEXT,
  row_version INTEGER NOT NULL DEFAULT 1 CHECK (row_version > 0),
  CHECK (parent_task_id IS NULL OR parent_task_id <> id)
) STRICT;

CREATE INDEX IF NOT EXISTS ix_tasks_parent ON tasks(parent_task_id);
CREATE INDEX IF NOT EXISTS ix_tasks_status ON tasks(status, updated_at_utc);

CREATE TABLE IF NOT EXISTS task_day_instances (
  id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE RESTRICT,
  work_date TEXT NOT NULL CHECK (work_date GLOB '????-??-??'),
  parent_instance_id TEXT REFERENCES task_day_instances(id) ON DELETE RESTRICT,
  carry_from_instance_id TEXT REFERENCES task_day_instances(id) ON DELETE RESTRICT,
  display_code TEXT NOT NULL,
  top_level_no INTEGER NOT NULL CHECK (top_level_no > 0),
  child_no INTEGER CHECK (child_no IS NULL OR child_no > 0),
  importance TEXT NOT NULL CHECK (importance IN ('important','secondary')),
  urgency TEXT NOT NULL CHECK (urgency IN ('urgent','relaxed')),
  day_status TEXT NOT NULL DEFAULT 'not_started' CHECK (day_status IN ('not_started','in_progress','waiting','blocked','completed','deferred','cancelled')),
  planned_start_minute INTEGER CHECK (planned_start_minute IS NULL OR planned_start_minute BETWEEN 0 AND 1439),
  planned_end_minute INTEGER CHECK (planned_end_minute IS NULL OR planned_end_minute BETWEEN 1 AND 1440),
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at_utc TEXT NOT NULL,
  updated_at_utc TEXT NOT NULL,
  UNIQUE(task_id, work_date),
  UNIQUE(work_date, display_code),
  CHECK ((planned_start_minute IS NULL AND planned_end_minute IS NULL) OR (planned_start_minute IS NOT NULL AND planned_end_minute IS NOT NULL AND planned_start_minute < planned_end_minute))
) STRICT;

CREATE INDEX IF NOT EXISTS ix_task_day_quadrant ON task_day_instances(work_date, importance, urgency, day_status);
CREATE INDEX IF NOT EXISTS ix_task_day_parent ON task_day_instances(parent_instance_id);

CREATE TABLE IF NOT EXISTS focus_sessions (
  id TEXT PRIMARY KEY,
  work_date TEXT NOT NULL CHECK (work_date GLOB '????-??-??'),
  status TEXT NOT NULL CHECK (status IN ('running','paused','completed','abandoned')),
  primary_task_instance_id TEXT REFERENCES task_day_instances(id) ON DELETE RESTRICT,
  planned_seconds INTEGER NOT NULL CHECK (planned_seconds > 0),
  remaining_seconds INTEGER NOT NULL CHECK (remaining_seconds >= 0),
  target_end_at_utc TEXT,
  started_at_utc TEXT NOT NULL,
  ended_at_utc TEXT,
  active_guard INTEGER UNIQUE CHECK (active_guard IS NULL OR active_guard = 1),
  CHECK ((status IN ('running','paused') AND active_guard = 1) OR (status NOT IN ('running','paused') AND active_guard IS NULL))
) STRICT;

CREATE TABLE IF NOT EXISTS focus_segments (
  id TEXT PRIMARY KEY,
  focus_session_id TEXT NOT NULL REFERENCES focus_sessions(id) ON DELETE CASCADE,
  task_instance_id TEXT NOT NULL REFERENCES task_day_instances(id) ON DELETE RESTRICT,
  started_at_utc TEXT NOT NULL,
  ended_at_utc TEXT,
  allocated_seconds INTEGER NOT NULL DEFAULT 0 CHECK (allocated_seconds >= 0)
) STRICT;

CREATE TABLE IF NOT EXISTS work_entries (
  id TEXT PRIMARY KEY,
  work_date TEXT NOT NULL CHECK (work_date GLOB '????-??-??'),
  focus_session_id TEXT REFERENCES focus_sessions(id) ON DELETE SET NULL,
  task_instance_id TEXT REFERENCES task_day_instances(id) ON DELETE SET NULL,
  entry_type TEXT NOT NULL CHECK (entry_type IN ('progress','idea','decision','blocker','result')),
  review_level TEXT NOT NULL CHECK (review_level IN ('key','normal','scratch')),
  content_md TEXT NOT NULL CHECK (length(trim(content_md)) > 0),
  occurred_at_utc TEXT NOT NULL,
  created_at_utc TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS events (
  seq INTEGER PRIMARY KEY AUTOINCREMENT,
  event_id TEXT NOT NULL UNIQUE,
  event_type TEXT NOT NULL,
  aggregate_type TEXT NOT NULL,
  aggregate_id TEXT NOT NULL,
  work_date TEXT NOT NULL CHECK (work_date GLOB '????-??-??'),
  occurred_at_utc TEXT NOT NULL,
  actor_type TEXT NOT NULL CHECK (actor_type IN ('user','system','recovery')),
  transaction_id TEXT NOT NULL,
  default_visibility TEXT NOT NULL CHECK (default_visibility IN ('summary','detail','hidden')),
  payload_json TEXT NOT NULL CHECK (json_valid(payload_json))
) STRICT;

CREATE INDEX IF NOT EXISTS ix_events_date_seq ON events(work_date, seq);
CREATE INDEX IF NOT EXISTS ix_events_aggregate ON events(aggregate_type, aggregate_id, seq);

CREATE TRIGGER IF NOT EXISTS events_are_immutable_update BEFORE UPDATE ON events BEGIN
  SELECT RAISE(ABORT, 'events are immutable; append a compensating event');
END;
CREATE TRIGGER IF NOT EXISTS events_are_immutable_delete BEFORE DELETE ON events BEGIN
  SELECT RAISE(ABORT, 'events are immutable; append a compensating event');
END;

CREATE TABLE IF NOT EXISTS sync_jobs (
  id TEXT PRIMARY KEY,
  job_kind TEXT NOT NULL CHECK (job_kind IN ('daily_note','essay_index','backup')),
  dedupe_key TEXT NOT NULL UNIQUE,
  state TEXT NOT NULL DEFAULT 'pending' CHECK (state IN ('pending','running','succeeded','failed','cancelled')),
  payload_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(payload_json)),
  created_at_utc TEXT NOT NULL,
  updated_at_utc TEXT NOT NULL,
  last_error TEXT
) STRICT;

INSERT OR IGNORE INTO schema_migrations(version, name, applied_at_utc)
VALUES (1, 'initial_schema', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
