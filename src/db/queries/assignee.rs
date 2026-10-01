//! The one optional assignee of an issue (migration 060): the account, human
//! or bot, that is working it. Informational only; it never changes the
//! status or what counts as workable.

use std::collections::HashMap;

use rusqlite::{Connection, OptionalExtension, params};

use crate::db::models::{AssigneeCandidate, Assignment};
use crate::error::LificError;

/// Who may be assigned in a project: an active account that is an
/// administrator or a member, or a bot whose owner is one. Same bot-to-owner
/// rule as `authz::effective_user`.
const ELIGIBLE: &str = "u.is_active = 1 AND EXISTS (
        SELECT 1 FROM users g
         WHERE g.id IN (u.id, CASE WHEN u.is_bot = 1 THEN u.owner_id END)
           AND (g.is_admin = 1 OR EXISTS (
                SELECT 1 FROM project_members m
                 WHERE m.project_id = ?1 AND m.user_id = g.id)))";

/// The id of the account `username` (with or without a leading `@`) names,
/// refused unless it may be assigned in `project_id`.
pub fn resolve(conn: &Connection, project_id: i64, username: &str) -> Result<i64, LificError> {
    let name = username.trim().trim_start_matches('@');
    let found: Option<(i64, bool)> = conn
        .query_row(
            &format!(
                "SELECT u.id, {ELIGIBLE} FROM users u
                  WHERE u.username = ?2 COLLATE NOCASE AND u.is_active = 1"
            ),
            params![project_id, name],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    match found {
        None => Err(LificError::NotFound(format!(
            "no active user named '{name}'"
        ))),
        Some((_, false)) => Err(LificError::BadRequest(format!(
            "'{name}' is not a member of this project and cannot be assigned"
        ))),
        Some((id, true)) => Ok(id),
    }
}

/// The accounts the assignee picker offers, humans first.
pub fn candidates(
    conn: &Connection,
    project_id: i64,
) -> Result<Vec<AssigneeCandidate>, LificError> {
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT u.username, u.display_name, u.is_bot FROM users u
          WHERE {ELIGIBLE} ORDER BY u.is_bot, u.username COLLATE NOCASE"
    ))?;
    let rows = stmt.query_map(params![project_id], |row| {
        Ok(AssigneeCandidate {
            username: row.get(0)?,
            display_name: row.get(1)?,
            is_bot: row.get(2)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Assignees per issue id, in one round trip. Unassigned issues are absent.
pub fn assignments_by_issue(
    conn: &Connection,
    ids: &[i64],
) -> Result<HashMap<i64, Assignment>, LificError> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let placeholders: String = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let mut stmt = conn.prepare(&format!(
        "SELECT i.id, u.username, u.display_name, u.is_bot
           FROM issues i JOIN users u ON u.id = i.assignee_id
          WHERE i.id IN ({placeholders})"
    ))?;
    let rows = stmt.query_map(rusqlite::params_from_iter(ids), |row| {
        Ok((
            row.get::<_, i64>(0)?,
            Assignment {
                assignee: Some(row.get(1)?),
                assignee_display_name: Some(row.get(2)?),
                assignee_is_bot: row.get(3)?,
            },
        ))
    })?;
    rows.collect::<Result<HashMap<_, _>, _>>()
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use crate::db::models::*;
    use crate::db::queries;
    use crate::error::LificError;

    /// A project led by `lead`, with an admin, an outsider, and one bot each
    /// for the lead and the outsider.
    fn fixture() -> (crate::db::DbPool, i64) {
        let db = crate::db::open_memory().expect("test db");
        let project = {
            let conn = db.write().unwrap();
            conn.execute_batch(
                "INSERT INTO users(id,username,email,password_hash,display_name,is_admin,is_bot,owner_id)
                   VALUES (1,'root','root@t','x','Root',1,0,NULL),
                          (2,'lead','lead@t','x','Lead',0,0,NULL),
                          (3,'outsider','out@t','x','Out',0,0,NULL),
                          (4,'lead-bot','lb@t','x','Lead Bot',0,1,2),
                          (5,'stray-bot','sb@t','x','Stray Bot',0,1,3);",
            )
            .unwrap();
            queries::create_project(
                &conn,
                &CreateProject {
                    name: "Assign".into(),
                    identifier: "ASG".into(),
                    lead_user_id: Some(2),
                    ..Default::default()
                },
            )
            .unwrap()
            .id
        };
        (db, project)
    }

    fn assign(name: Option<&str>) -> UpdateIssue {
        UpdateIssue {
            assignee: Some(name.map(str::to_string)),
            ..Default::default()
        }
    }

    #[test]
    fn members_admins_and_their_bots_can_be_assigned_and_others_cannot() {
        let (db, project) = fixture();
        let conn = db.write().unwrap();
        let issue = queries::create_issue(
            &conn,
            &CreateIssue {
                project_id: project,
                title: "Carrier".into(),
                assignee: Some("lead-bot".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            issue.assignment,
            Assignment {
                assignee: Some("lead-bot".into()),
                assignee_display_name: Some("Lead Bot".into()),
                assignee_is_bot: true,
            }
        );

        for name in ["lead", "@LEAD", "root"] {
            let updated = queries::update_issue(&conn, issue.id, &assign(Some(name))).unwrap();
            assert!(!updated.assignment.assignee_is_bot, "{name}");
        }
        for name in ["outsider", "stray-bot"] {
            let error = queries::update_issue(&conn, issue.id, &assign(Some(name))).unwrap_err();
            assert!(
                matches!(error, LificError::BadRequest(_)),
                "{name}: {error}"
            );
        }
        let error = queries::update_issue(&conn, issue.id, &assign(Some("nobody"))).unwrap_err();
        assert!(matches!(error, LificError::NotFound(_)), "{error}");
        let refused = queries::create_issue(
            &conn,
            &CreateIssue {
                project_id: project,
                title: "Never".into(),
                assignee: Some("outsider".into()),
                ..Default::default()
            },
        );
        assert!(refused.is_err());
        assert_eq!(
            queries::get_issue(&conn, issue.id)
                .unwrap()
                .assignment
                .assignee
                .as_deref(),
            Some("root"),
            "a refused assignment changes nothing"
        );

        let names: Vec<String> = super::candidates(&conn, project)
            .unwrap()
            .into_iter()
            .map(|candidate| candidate.username)
            .collect();
        assert_eq!(names, ["lead", "root", "lead-bot"]);
    }

    #[test]
    fn assignment_is_audited_filterable_and_leaves_status_and_workable_alone() {
        let (db, project) = fixture();
        let conn = db.write().unwrap();
        let create = |title: &str| {
            queries::create_issue(
                &conn,
                &CreateIssue {
                    project_id: project,
                    title: title.into(),
                    status: Status::Todo,
                    ..Default::default()
                },
            )
            .unwrap()
        };
        let mine = create("Mine");
        create("Free");
        let assigned = queries::update_issue(&conn, mine.id, &assign(Some("lead"))).unwrap();
        assert!(assigned.seq > mine.seq, "an assignment is a change");
        assert_eq!(assigned.status, Status::Todo);
        // Assigning the same account again writes nothing.
        let again = queries::update_issue(&conn, mine.id, &assign(Some("lead"))).unwrap();
        assert_eq!(again.seq, assigned.seq);

        let list = |query: ListIssuesQuery| -> Vec<String> {
            queries::list_issues(&conn, &query)
                .unwrap()
                .into_iter()
                .map(|issue| issue.title)
                .collect()
        };
        let of = |assignee: &str| ListIssuesQuery {
            project_id: Some(project),
            assignee: Some(assignee.into()),
            ..Default::default()
        };
        assert_eq!(list(of("lead")), ["Mine"]);
        assert!(list(of("root")).is_empty());
        assert!(list(of("nobody")).is_empty());
        let workable = list(ListIssuesQuery {
            project_id: Some(project),
            workable: Some(true),
            ..Default::default()
        });
        assert_eq!(workable, ["Mine", "Free"]);
        let listed = queries::list_issues(&conn, &of("lead")).unwrap();
        assert_eq!(listed[0].assignment.assignee.as_deref(), Some("lead"));

        queries::update_issue(&conn, mine.id, &assign(None)).unwrap();
        let audit: Vec<(Option<String>, Option<String>)> = conn
            .prepare(
                "SELECT old_value, new_value FROM audit_log
                  WHERE issue_id = ?1 AND field = 'assignee' ORDER BY id",
            )
            .unwrap()
            .query_map([mine.id], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            audit,
            [
                (None, Some("lead".to_string())),
                (Some("lead".to_string()), None)
            ]
        );
    }

    #[test]
    fn deleting_the_account_clears_the_assignment() {
        let (db, project) = fixture();
        let conn = db.write().unwrap();
        let issue = queries::create_issue(
            &conn,
            &CreateIssue {
                project_id: project,
                title: "Carrier".into(),
                assignee: Some("lead-bot".into()),
                ..Default::default()
            },
        )
        .unwrap();
        conn.execute("DELETE FROM users WHERE id = 4", []).unwrap();
        let after = queries::get_issue(&conn, issue.id).unwrap();
        assert_eq!(after.assignment, Assignment::default());
        assert!(after.seq > issue.seq, "replicas hear about it");
    }
}
