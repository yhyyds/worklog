CREATE TABLE IF NOT EXISTS essay_notes (
  note_id TEXT PRIMARY KEY,
  relative_path TEXT NOT NULL UNIQUE,
  title TEXT NOT NULL CHECK (length(trim(title)) > 0),
  created_at_utc TEXT NOT NULL,
  updated_at_utc TEXT NOT NULL,
  row_version INTEGER NOT NULL DEFAULT 1 CHECK (row_version > 0)
) STRICT;

INSERT OR IGNORE INTO schema_migrations(version, name, applied_at_utc)
VALUES (4, 'essay_note_index', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
