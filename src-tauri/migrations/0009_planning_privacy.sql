CREATE TABLE IF NOT EXISTS growth_categories (
 id TEXT PRIMARY KEY, name TEXT NOT NULL UNIQUE CHECK(length(trim(name)) BETWEEN 1 AND 40),
 color TEXT NOT NULL, share_mode TEXT NOT NULL DEFAULT 'anonymous' CHECK(share_mode IN ('public','anonymous','excluded'))
) STRICT;
CREATE TABLE IF NOT EXISTS growth_classifications (
 entity_id TEXT PRIMARY KEY, entity_kind TEXT NOT NULL CHECK(entity_kind IN ('habit','goal')),
 category_id TEXT REFERENCES growth_categories(id) ON DELETE RESTRICT,
 share_name INTEGER NOT NULL DEFAULT 0 CHECK(share_name IN (0,1))
) STRICT;
CREATE TABLE IF NOT EXISTS goal_action_options (
 action_id TEXT PRIMARY KEY REFERENCES goal_actions(id),
 importance TEXT NOT NULL DEFAULT 'important' CHECK(importance IN ('important','secondary')),
 urgency TEXT NOT NULL DEFAULT 'relaxed' CHECK(urgency IN ('urgent','relaxed')),
 deleted_at TEXT
) STRICT;
CREATE TABLE IF NOT EXISTS goal_action_occurrences (
 id TEXT PRIMARY KEY, action_id TEXT NOT NULL REFERENCES goal_actions(id),
 task_id TEXT NOT NULL UNIQUE REFERENCES tasks(id), scheduled_date TEXT NOT NULL,
 action_kind TEXT NOT NULL CHECK(action_kind IN ('one_off','repeating')),
 active INTEGER NOT NULL DEFAULT 1 CHECK(active IN (0,1))
) STRICT;
CREATE UNIQUE INDEX IF NOT EXISTS ix_goal_occurrence_date ON goal_action_occurrences(action_id,scheduled_date) WHERE active=1;
CREATE TABLE IF NOT EXISTS goal_removed_instances (instance_id TEXT PRIMARY KEY REFERENCES task_day_instances(id)) STRICT;
INSERT OR IGNORE INTO schema_migrations VALUES(9,'planning_privacy',strftime('%Y-%m-%dT%H:%M:%fZ','now'));
