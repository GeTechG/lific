-- One optional assignee per issue: the account (human or bot) working it.
-- Informational only: it changes neither the status nor what counts as
-- workable.
--
-- A plain ADD COLUMN, so the issues table is not rebuilt. Deleting the
-- account clears the assignment, like `projects.lead_user_id`.

ALTER TABLE issues ADD COLUMN assignee_id INTEGER REFERENCES users(id) ON DELETE SET NULL;

-- ── Audit: an assignment change is a field change on the issue ────────
-- Stored by username, as 018 stores a module by name. The seq and
-- updated_at bumps come from the existing AFTER UPDATE triggers on issues.

CREATE TRIGGER IF NOT EXISTS audit_issues_assignee AFTER UPDATE OF assignee_id ON issues
WHEN OLD.assignee_id IS NOT NEW.assignee_id
BEGIN
    INSERT INTO audit_log (actor_user_id, transport, entity_type, entity_id, entity_label,
                           project_id, issue_id, action, field, old_value, new_value)
    VALUES (
        (SELECT user_id FROM _actor_state WHERE id = 1),
        COALESCE((SELECT transport FROM _actor_state WHERE id = 1), 'system'),
        'issue', NEW.id,
        (SELECT identifier FROM projects WHERE id = NEW.project_id) || '-' || NEW.sequence,
        NEW.project_id, NEW.id, 'update', 'assignee',
        (SELECT username FROM users WHERE id = OLD.assignee_id),
        (SELECT username FROM users WHERE id = NEW.assignee_id)
    );
END;
