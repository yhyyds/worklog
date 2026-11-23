CREATE TABLE IF NOT EXISTS long_term_work (
  id TEXT PRIMARY KEY,
  parent_id TEXT REFERENCES long_term_work(id),
  title TEXT NOT NULL CHECK (length(trim(title)) BETWEEN 1 AND 200),
  workload TEXT CHECK (workload IN ('light', 'large', 'very_large')),
  completed INTEGER NOT NULL DEFAULT 0 CHECK (completed IN (0, 1)),
  CHECK (parent_id IS NOT NULL OR workload IS NULL)
) STRICT;

CREATE INDEX IF NOT EXISTS ix_long_term_work_parent ON long_term_work(parent_id);

CREATE TABLE IF NOT EXISTS task_workloads (
  task_id TEXT PRIMARY KEY REFERENCES tasks(id),
  workload TEXT CHECK (workload IN ('light', 'large', 'very_large'))
) STRICT;

INSERT OR IGNORE INTO schema_migrations(version, name, applied_at_utc)
VALUES (11, 'long_term_work', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
