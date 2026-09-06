CREATE TABLE IF NOT EXISTS task_inbox (
  task_id TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE RESTRICT,
  group_id TEXT NOT NULL,
  source_instance_id TEXT REFERENCES task_day_instances(id) ON DELETE RESTRICT,
  parent_task_id TEXT REFERENCES tasks(id) ON DELETE RESTRICT,
  importance TEXT NOT NULL CHECK (importance IN ('important','secondary')),
  urgency TEXT NOT NULL CHECK (urgency IN ('urgent','relaxed')),
  entered_at_utc TEXT NOT NULL
) STRICT;

-- Preserve original instances and events while removing their plan membership.
CREATE TABLE IF NOT EXISTS inbox_removed_instances (
  instance_id TEXT PRIMARY KEY REFERENCES task_day_instances(id) ON DELETE RESTRICT,
  removed_at_utc TEXT NOT NULL
) STRICT;

INSERT OR IGNORE INTO schema_migrations(version,name,applied_at_utc)
VALUES(8,'task_inbox',strftime('%Y-%m-%dT%H:%M:%fZ','now'));
