CREATE TABLE IF NOT EXISTS habits (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL CHECK (length(trim(title)) > 0),
  start_date TEXT NOT NULL CHECK (start_date GLOB '????-??-??'),
  active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0,1)),
  archived_date TEXT CHECK (archived_date IS NULL OR archived_date GLOB '????-??-??'),
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at_utc TEXT NOT NULL,
  updated_at_utc TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS habit_dependencies (
  habit_id TEXT NOT NULL REFERENCES habits(id) ON DELETE CASCADE,
  prerequisite_habit_id TEXT NOT NULL REFERENCES habits(id) ON DELETE RESTRICT,
  PRIMARY KEY (habit_id, prerequisite_habit_id),
  CHECK (habit_id <> prerequisite_habit_id)
) STRICT;

CREATE TABLE IF NOT EXISTS habit_reviews (
  review_date TEXT PRIMARY KEY CHECK (review_date GLOB '????-??-??'),
  reviewed_at_utc TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS habit_occurrences (
  habit_id TEXT NOT NULL REFERENCES habits(id) ON DELETE RESTRICT,
  occurrence_date TEXT NOT NULL CHECK (occurrence_date GLOB '????-??-??'),
  raw_completed INTEGER NOT NULL CHECK (raw_completed IN (0,1)),
  effective_completed INTEGER NOT NULL CHECK (effective_completed IN (0,1)),
  dependency_snapshot_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(dependency_snapshot_json)),
  reviewed_at_utc TEXT NOT NULL,
  PRIMARY KEY (habit_id, occurrence_date)
) STRICT;

CREATE INDEX IF NOT EXISTS ix_habit_occurrences_date
ON habit_occurrences(occurrence_date, effective_completed);

CREATE TABLE IF NOT EXISTS long_term_goals (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL CHECK (length(trim(title)) > 0),
  description_md TEXT NOT NULL DEFAULT '',
  cycle_days INTEGER NOT NULL CHECK (cycle_days BETWEEN 1 AND 3660),
  start_date TEXT NOT NULL CHECK (start_date GLOB '????-??-??'),
  status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active','completed','archived')),
  created_at_utc TEXT NOT NULL,
  updated_at_utc TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS goal_phases (
  id TEXT PRIMARY KEY,
  goal_id TEXT NOT NULL REFERENCES long_term_goals(id) ON DELETE CASCADE,
  title TEXT NOT NULL CHECK (length(trim(title)) > 0),
  start_date TEXT NOT NULL CHECK (start_date GLOB '????-??-??'),
  end_date TEXT NOT NULL CHECK (end_date GLOB '????-??-??'),
  brainstorm_md TEXT NOT NULL DEFAULT '',
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at_utc TEXT NOT NULL,
  updated_at_utc TEXT NOT NULL,
  CHECK (start_date <= end_date)
) STRICT;

CREATE TABLE IF NOT EXISTS goal_actions (
  id TEXT PRIMARY KEY,
  phase_id TEXT NOT NULL REFERENCES goal_phases(id) ON DELETE CASCADE,
  title TEXT NOT NULL CHECK (length(trim(title)) > 0),
  action_kind TEXT NOT NULL CHECK (action_kind IN ('one_off','repeating')),
  required INTEGER NOT NULL DEFAULT 1 CHECK (required IN (0,1)),
  target_count INTEGER NOT NULL DEFAULT 1 CHECK (target_count > 0),
  completed_count INTEGER NOT NULL DEFAULT 0 CHECK (completed_count >= 0),
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at_utc TEXT NOT NULL,
  updated_at_utc TEXT NOT NULL
) STRICT;

CREATE INDEX IF NOT EXISTS ix_goal_phases_goal ON goal_phases(goal_id, sort_order);
CREATE INDEX IF NOT EXISTS ix_goal_actions_phase ON goal_actions(phase_id, sort_order);

CREATE TABLE IF NOT EXISTS quote_usage (
  quote_id TEXT NOT NULL,
  week_start TEXT NOT NULL CHECK (week_start GLOB '????-??-??'),
  scenario TEXT NOT NULL,
  used_at_utc TEXT NOT NULL,
  PRIMARY KEY (quote_id, week_start)
) STRICT;

INSERT OR IGNORE INTO schema_migrations(version, name, applied_at_utc)
VALUES (7, 'personal_growth_system', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
