-- Add `in_review` to the issue status set, between `active` and `done`.
-- The issues table is rebuilt because SQLite cannot alter its CHECK
-- constraint in place. Ids are preserved verbatim, so every child table's
-- reference stays valid; the migration runner preserves existing triggers
-- around this table rebuild.
CREATE TABLE issues_status_rebuild (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id  INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    sequence    INTEGER NOT NULL,
    title       TEXT    NOT NULL,
    description TEXT    NOT NULL DEFAULT '',
    status      TEXT    NOT NULL DEFAULT 'backlog'
                        CHECK(status IN ('backlog','todo','active','in_review','done','cancelled')),
    priority    TEXT    NOT NULL DEFAULT 'none'
                        CHECK(priority IN ('urgent','high','medium','low','none')),
    module_id   INTEGER REFERENCES modules(id) ON DELETE SET NULL,
    sort_order  REAL    NOT NULL DEFAULT 0,
    start_date  TEXT,
    target_date TEXT,
    created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    source      TEXT,
    seq         INTEGER,
    deleted_at  TEXT,
    UNIQUE(project_id, sequence)
);
INSERT INTO issues_status_rebuild
    (id, project_id, sequence, title, description, status, priority, module_id,
     sort_order, start_date, target_date, created_at, updated_at, source, seq, deleted_at)
SELECT id, project_id, sequence, title, description, status, priority, module_id,
       sort_order, start_date, target_date, created_at, updated_at, source, seq, deleted_at
FROM issues;
-- Preserve purged ids too: audit history can still refer to them.
INSERT INTO sqlite_sequence (name, seq)
SELECT 'issues_status_rebuild', seq FROM sqlite_sequence
WHERE name = 'issues'
  AND NOT EXISTS (SELECT 1 FROM sqlite_sequence WHERE name = 'issues_status_rebuild');
UPDATE sqlite_sequence
SET seq = max(seq, COALESCE((SELECT seq FROM sqlite_sequence WHERE name = 'issues'), 0))
WHERE name = 'issues_status_rebuild';
DROP TABLE issues;
ALTER TABLE issues_status_rebuild RENAME TO issues;
CREATE INDEX idx_issues_project     ON issues(project_id);
CREATE INDEX idx_issues_status      ON issues(project_id, status);
CREATE INDEX idx_issues_priority    ON issues(project_id, priority);
CREATE INDEX idx_issues_module      ON issues(module_id);
CREATE INDEX idx_issues_project_seq ON issues(project_id, seq DESC);
CREATE INDEX idx_issues_deleted_at
    ON issues(deleted_at) WHERE deleted_at IS NOT NULL;
CREATE UNIQUE INDEX idx_issues_source
    ON issues(source) WHERE source IS NOT NULL AND deleted_at IS NULL;
