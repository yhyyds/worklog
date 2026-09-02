CREATE TABLE IF NOT EXISTS daily_note_sync (
  work_date TEXT PRIMARY KEY CHECK (work_date GLOB '????-??-??'),
  relative_path TEXT NOT NULL UNIQUE,
  sync_state TEXT NOT NULL DEFAULT 'dirty' CHECK (sync_state IN ('clean','dirty','writing','conflict','error')),
  last_generated_hash TEXT,
  last_error TEXT,
  last_attempt_at_utc TEXT,
  last_success_at_utc TEXT,
  row_version INTEGER NOT NULL DEFAULT 1 CHECK (row_version > 0)
) STRICT;

INSERT OR IGNORE INTO schema_migrations(version, name, applied_at_utc)
VALUES (2, 'obsidian_daily_note_sync', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
