-- A per-issue run log: an append-only stream of short text lines a scheduler
-- writes while an agent works the issue, shown live in the UI. It is neither
-- a comment nor audit history.
--
-- Deliberately no triggers. A log line must not advance the issue's seq or
-- updated_at, appear in /changes, the audit log, search or the briefing:
-- an agent writes one every few seconds, and each bump would fail the
-- optimistic-concurrency check of whoever is editing the issue. Lines live
-- only here and leave with the issue when the purge deletes it.
--
-- AUTOINCREMENT keeps ids increasing even after the oldest lines are
-- trimmed, so `after=<id>` paging never sees an id reused.

CREATE TABLE IF NOT EXISTS issue_log (
    id       INTEGER PRIMARY KEY AUTOINCREMENT,
    issue_id INTEGER NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    ts       TEXT    NOT NULL DEFAULT (datetime('now')),
    source   TEXT    NOT NULL DEFAULT '',
    text     TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_issue_log_issue ON issue_log(issue_id, id);
