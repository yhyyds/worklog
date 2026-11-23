CREATE TABLE IF NOT EXISTS long_term_transfers (
  task_id TEXT PRIMARY KEY REFERENCES tasks(id),
  source_instance_id TEXT NOT NULL REFERENCES task_day_instances(id),
  work_date TEXT NOT NULL,
  FOREIGN KEY (task_id) REFERENCES long_term_work(id)
) STRICT;

CREATE VIEW IF NOT EXISTS long_term_removed_instances AS
SELECT d.id AS instance_id
FROM task_day_instances d
JOIN long_term_transfers m ON m.task_id=d.task_id AND d.work_date>=m.work_date;

CREATE TABLE IF NOT EXISTS long_term_focus_targets (
  task_id TEXT PRIMARY KEY REFERENCES tasks(id),
  item_id TEXT NOT NULL UNIQUE REFERENCES long_term_work(id)
) STRICT;

INSERT OR IGNORE INTO schema_migrations(version, name, applied_at_utc)
VALUES (12, 'long_term_transfers', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
