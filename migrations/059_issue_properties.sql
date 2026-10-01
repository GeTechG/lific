-- Free-form text properties on issues: per-issue name -> value pairs with no
-- per-project schema. A scheduler or an agent keeps a value such as a path
-- footprint here instead of as a line inside the description.
--
-- Names are `[a-z0-9][a-z0-9_-]{0,63}`; the application also caps a value's
-- size (see `queries::properties`). An empty value is not stored: unsetting
-- deletes the row, and the audit trigger below keeps the history.
--
-- Like labels and waits, properties survive an issue's tombstone so a restore
-- brings them back, and the purge's physical DELETE cascades them away.

CREATE TABLE IF NOT EXISTS issue_properties (
    issue_id    INTEGER NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    name        TEXT    NOT NULL
                CHECK (length(name) <= 64
                       AND name GLOB '[a-z0-9]*'
                       AND name NOT GLOB '*[^a-z0-9_-]*'),
    value       TEXT    NOT NULL CHECK (value <> ''),
    created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (issue_id, name)
);

-- ── Sync: a property change is activity on its issue ──────────────────
-- Same shape as 045's label bumps and 054's wait bumps: the issue's seq and
-- updated_at advance, so `/changes` re-delivers the issue row.

CREATE TRIGGER IF NOT EXISTS issue_properties_bump_ai
AFTER INSERT ON issue_properties
BEGIN
    UPDATE sync_seq SET value = value + 1 WHERE id = 1;
    UPDATE issues
       SET updated_at = datetime('now'),
           seq = (SELECT value FROM sync_seq WHERE id = 1)
     WHERE id = NEW.issue_id;
END;

CREATE TRIGGER IF NOT EXISTS issue_properties_bump_au
AFTER UPDATE OF value ON issue_properties
BEGIN
    UPDATE sync_seq SET value = value + 1 WHERE id = 1;
    UPDATE issues
       SET updated_at = datetime('now'),
           seq = (SELECT value FROM sync_seq WHERE id = 1)
     WHERE id = NEW.issue_id;
END;

CREATE TRIGGER IF NOT EXISTS issue_properties_bump_ad
AFTER DELETE ON issue_properties
BEGIN
    UPDATE sync_seq SET value = value + 1 WHERE id = 1;
    UPDATE issues
       SET updated_at = datetime('now'),
           seq = (SELECT value FROM sync_seq WHERE id = 1)
     WHERE id = OLD.issue_id;
END;

-- ── Audit: one 'update' row per changed property ──────────────────────
-- Recorded against the issue like any other field change, with
-- `field = 'property:<name>'`. Setting a new property has no old value and
-- unsetting one has no new value.

CREATE TRIGGER IF NOT EXISTS audit_issue_properties_set AFTER INSERT ON issue_properties BEGIN
    INSERT INTO audit_log (actor_user_id, transport, entity_type, entity_id, entity_label,
                           project_id, issue_id, action, field, new_value)
    VALUES (
        (SELECT user_id FROM _actor_state WHERE id = 1),
        COALESCE((SELECT transport FROM _actor_state WHERE id = 1), 'system'),
        'issue', NEW.issue_id,
        (SELECT p.identifier || '-' || i.sequence FROM issues i JOIN projects p ON p.id = i.project_id WHERE i.id = NEW.issue_id),
        (SELECT project_id FROM issues WHERE id = NEW.issue_id),
        NEW.issue_id, 'update', 'property:' || NEW.name, NEW.value
    );
END;

CREATE TRIGGER IF NOT EXISTS audit_issue_properties_change AFTER UPDATE OF value ON issue_properties
WHEN OLD.value IS NOT NEW.value
BEGIN
    INSERT INTO audit_log (actor_user_id, transport, entity_type, entity_id, entity_label,
                           project_id, issue_id, action, field, old_value, new_value)
    VALUES (
        (SELECT user_id FROM _actor_state WHERE id = 1),
        COALESCE((SELECT transport FROM _actor_state WHERE id = 1), 'system'),
        'issue', NEW.issue_id,
        (SELECT p.identifier || '-' || i.sequence FROM issues i JOIN projects p ON p.id = i.project_id WHERE i.id = NEW.issue_id),
        (SELECT project_id FROM issues WHERE id = NEW.issue_id),
        NEW.issue_id, 'update', 'property:' || NEW.name, OLD.value, NEW.value
    );
END;

-- Silent for the purge's cascade: the issue is no longer live by then.
CREATE TRIGGER IF NOT EXISTS audit_issue_properties_unset AFTER DELETE ON issue_properties
WHEN EXISTS (SELECT 1 FROM issues WHERE id = OLD.issue_id AND deleted_at IS NULL)
BEGIN
    INSERT INTO audit_log (actor_user_id, transport, entity_type, entity_id, entity_label,
                           project_id, issue_id, action, field, old_value)
    VALUES (
        (SELECT user_id FROM _actor_state WHERE id = 1),
        COALESCE((SELECT transport FROM _actor_state WHERE id = 1), 'system'),
        'issue', OLD.issue_id,
        (SELECT p.identifier || '-' || i.sequence FROM issues i JOIN projects p ON p.id = i.project_id WHERE i.id = OLD.issue_id),
        (SELECT project_id FROM issues WHERE id = OLD.issue_id),
        OLD.issue_id, 'update', 'property:' || OLD.name, OLD.value
    );
END;
