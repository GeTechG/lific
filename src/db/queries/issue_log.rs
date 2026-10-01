//! The per-issue run log (migration 061): short text lines a scheduler
//! appends while an agent works an issue.
//!
//! Nothing here touches the `issues` row. Appending leaves the issue's
//! `seq` and `updated_at` alone, so it never shows up in `/changes`, the
//! audit log or search, and never fails somebody's `expected_seq`.

use std::collections::HashMap;

use rusqlite::{Connection, params};

use crate::db::models::IssueLogLine;
use crate::error::LificError;

/// Longest stored line, in characters. Longer ones are cut.
pub const MAX_LINE_CHARS: usize = 2000;

/// Lines one issue keeps. Appending past it drops the oldest.
pub const MAX_LINES_PER_ISSUE: usize = 2000;

/// Longest `source` label, in characters.
pub const MAX_SOURCE_CHARS: usize = 64;

/// Lines a read returns when the caller names no limit.
pub const DEFAULT_READ_LIMIT: i64 = 200;

fn truncated(text: &str, max_chars: usize) -> &str {
    match text.char_indices().nth(max_chars) {
        Some((end, _)) => &text[..end],
        None => text,
    }
}

/// Append `lines` to an issue's log and return them as stored. Empty lines
/// are skipped, long ones cut at [`MAX_LINE_CHARS`], and the issue is
/// trimmed to its newest [`MAX_LINES_PER_ISSUE`].
pub fn append(
    conn: &Connection,
    issue_id: i64,
    source: &str,
    lines: &[String],
) -> Result<Vec<IssueLogLine>, LificError> {
    let source = truncated(source.trim(), MAX_SOURCE_CHARS);
    let kept: Vec<&str> = lines
        .iter()
        .map(|line| truncated(line.trim_end_matches(['\r', '\n']), MAX_LINE_CHARS))
        .filter(|line| !line.trim().is_empty())
        .collect();
    // More than the cap in one call: only the tail could survive the trim.
    let kept = &kept[kept.len().saturating_sub(MAX_LINES_PER_ISSUE)..];
    let Some(first) = kept.first() else {
        return Ok(Vec::new());
    };
    super::savepoint(conn, "append_issue_log", || {
        let mut insert = conn
            .prepare_cached("INSERT INTO issue_log (issue_id, source, text) VALUES (?1, ?2, ?3)")?;
        insert.execute(params![issue_id, source, first])?;
        let first_id = conn.last_insert_rowid();
        for line in &kept[1..] {
            insert.execute(params![issue_id, source, line])?;
        }
        conn.execute(
            "DELETE FROM issue_log
              WHERE issue_id = ?1
                AND id <= (SELECT id FROM issue_log WHERE issue_id = ?1
                            ORDER BY id DESC LIMIT 1 OFFSET ?2)",
            params![issue_id, MAX_LINES_PER_ISSUE as i64],
        )?;
        select(
            conn,
            "issue_id = ?1 AND id >= ?2 ORDER BY id",
            params![issue_id, first_id],
        )
    })
}

/// Which lines a read wants.
#[derive(Debug, Clone, Copy, Default)]
pub struct LogWindow {
    /// Only lines after this id, oldest first: paging forward.
    pub after: Option<i64>,
    /// Only lines before this id, the newest of them: paging back.
    pub before: Option<i64>,
    pub limit: Option<i64>,
}

/// An issue's log lines in chronological order. With `after`, the next
/// `limit` lines past that id; otherwise the newest `limit` (before
/// `before`, when given).
pub fn list(
    conn: &Connection,
    issue_id: i64,
    window: LogWindow,
) -> Result<Vec<IssueLogLine>, LificError> {
    let limit = window
        .limit
        .unwrap_or(DEFAULT_READ_LIMIT)
        .clamp(1, MAX_LINES_PER_ISSUE as i64);
    if let Some(after) = window.after {
        return select(
            conn,
            "issue_id = ?1 AND id > ?2 ORDER BY id LIMIT ?3",
            params![issue_id, after, limit],
        );
    }
    let mut lines = select(
        conn,
        "issue_id = ?1 AND id < ?2 ORDER BY id DESC LIMIT ?3",
        params![issue_id, window.before.unwrap_or(i64::MAX), limit],
    )?;
    lines.reverse();
    Ok(lines)
}

fn select(
    conn: &Connection,
    tail: &str,
    params: impl rusqlite::Params,
) -> Result<Vec<IssueLogLine>, LificError> {
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT id, ts, source, text FROM issue_log WHERE {tail}"
    ))?;
    let rows = stmt.query_map(params, |row| {
        Ok(IssueLogLine {
            id: row.get(0)?,
            ts: row.get(1)?,
            source: row.get(2)?,
            text: row.get(3)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// When each issue last logged a line, in one round trip (the list and
/// `/changes` pages). Issues with no lines are absent.
pub fn last_log_at_by_issue(
    conn: &Connection,
    ids: &[i64],
) -> Result<HashMap<i64, String>, LificError> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let placeholders: String = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let mut stmt = conn.prepare(&format!(
        "SELECT issue_id, ts FROM issue_log
          WHERE id IN (SELECT MAX(id) FROM issue_log
                        WHERE issue_id IN ({placeholders}) GROUP BY issue_id)"
    ))?;
    let rows = stmt.query_map(rusqlite::params_from_iter(ids), |row| {
        Ok((row.get(0)?, row.get(1)?))
    })?;
    rows.collect::<Result<HashMap<_, _>, _>>()
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::*;
    use crate::db::queries;

    fn fixture() -> (crate::db::DbPool, i64, i64) {
        let db = crate::db::open_memory().expect("test db");
        let (project, issue) = {
            let conn = db.write().unwrap();
            let project = queries::create_project(
                &conn,
                &CreateProject {
                    name: "Runs".into(),
                    identifier: "RUN".into(),
                    ..Default::default()
                },
            )
            .unwrap()
            .id;
            let issue = queries::create_issue(
                &conn,
                &CreateIssue {
                    project_id: project,
                    title: "Worked on".into(),
                    ..Default::default()
                },
            )
            .unwrap()
            .id;
            (project, issue)
        };
        (db, project, issue)
    }

    fn owned(lines: &[&str]) -> Vec<String> {
        lines.iter().map(|line| (*line).to_string()).collect()
    }

    fn count(conn: &Connection, sql: &str) -> i64 {
        conn.query_row(sql, [], |row| row.get(0)).unwrap()
    }

    /// The whole point of the table: an agent logging a line every few
    /// seconds must not look like an edit to anybody.
    #[test]
    fn appending_leaves_the_issue_row_and_every_feed_alone() {
        let (db, project, issue) = fixture();
        let conn = db.write().unwrap();
        // Make the stored updated_at distinguishable from "now".
        conn.execute_batch(
            "DROP TRIGGER issues_updated;
             UPDATE issues SET updated_at = '2020-01-01 00:00:00';",
        )
        .unwrap();
        let before = queries::get_issue(&conn, issue).unwrap();
        let audit = count(&conn, "SELECT count(*) FROM audit_log");
        let search = count(&conn, "SELECT count(*) FROM search_index");
        let counter = count(&conn, "SELECT value FROM sync_seq WHERE id = 1");

        let appended =
            append(&conn, issue, "run-7", &owned(&["cloning", "", "testing\n"])).unwrap();
        assert_eq!(appended.len(), 2, "the empty line is skipped");
        assert_eq!(appended[1].text, "testing");
        assert_eq!(appended[0].source, "run-7");

        let after = queries::get_issue(&conn, issue).unwrap();
        assert_eq!(after.seq, before.seq);
        assert_eq!(after.updated_at, "2020-01-01 00:00:00");
        assert_eq!(count(&conn, "SELECT count(*) FROM audit_log"), audit);
        assert_eq!(count(&conn, "SELECT count(*) FROM search_index"), search);
        assert_eq!(
            count(&conn, "SELECT value FROM sync_seq WHERE id = 1"),
            counter
        );
        let changes = queries::changes::list_changes(&conn, project, before.seq, 100).unwrap();
        assert!(changes.changes.is_empty(), "nothing to re-deliver");
        assert_eq!(
            count(
                &conn,
                "SELECT count(*) FROM sqlite_master WHERE type='trigger' AND tbl_name='issue_log'"
            ),
            0
        );

        // The one thing a read does learn is when the log last moved.
        assert_eq!(before.last_log_at, None);
        assert_eq!(after.last_log_at.as_deref(), Some(appended[1].ts.as_str()));
        let listed = queries::list_issues(&conn, &ListIssuesQuery::default()).unwrap();
        assert_eq!(listed[0].last_log_at, after.last_log_at);

        // And an edit guarded by the seq read before the lines still lands.
        queries::update_issue(
            &conn,
            issue,
            &UpdateIssue {
                title: Some("Edited".into()),
                expected_seq: Some(before.seq),
                ..Default::default()
            },
        )
        .unwrap();
    }

    #[test]
    fn long_lines_are_cut_and_only_the_newest_lines_are_kept() {
        let (db, _, issue) = fixture();
        let conn = db.write().unwrap();
        let long = "é".repeat(MAX_LINE_CHARS + 5);
        let stored = append(&conn, issue, &"s".repeat(100), &[long]).unwrap();
        assert_eq!(stored[0].text.chars().count(), MAX_LINE_CHARS);
        assert_eq!(stored[0].source.len(), MAX_SOURCE_CHARS);

        let many: Vec<String> = (1..=MAX_LINES_PER_ISSUE + 100)
            .map(|n| format!("line {n}"))
            .collect();
        let returned = append(&conn, issue, "", &many).unwrap();
        assert_eq!(returned.len(), MAX_LINES_PER_ISSUE);
        assert_eq!(
            count(&conn, "SELECT count(*) FROM issue_log"),
            MAX_LINES_PER_ISSUE as i64
        );
        let all = list(
            &conn,
            issue,
            LogWindow {
                limit: Some(i64::MAX),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(all.first().unwrap().text, "line 101");
        assert_eq!(all.last().unwrap().text, "line 2100");

        // One more line pushes exactly one out, and ids keep increasing.
        let newest = append(&conn, issue, "", &owned(&["tail"])).unwrap();
        assert!(newest[0].id > all.last().unwrap().id);
        assert_eq!(
            count(&conn, "SELECT count(*) FROM issue_log"),
            MAX_LINES_PER_ISSUE as i64
        );
        assert_eq!(
            count(
                &conn,
                "SELECT count(*) FROM issue_log WHERE text = 'line 101'"
            ),
            0
        );
    }

    #[test]
    fn reads_page_forward_and_back_in_chronological_order() {
        let (db, _, issue) = fixture();
        let conn = db.write().unwrap();
        let lines: Vec<String> = (1..=10).map(|n| format!("l{n}")).collect();
        let stored = append(&conn, issue, "", &lines).unwrap();
        let texts = |window: LogWindow| -> Vec<String> {
            list(&conn, issue, window)
                .unwrap()
                .into_iter()
                .map(|line| line.text)
                .collect()
        };
        let newest = texts(LogWindow {
            limit: Some(3),
            ..Default::default()
        });
        assert_eq!(newest, ["l8", "l9", "l10"]);
        let forward = texts(LogWindow {
            after: Some(stored[1].id),
            limit: Some(2),
            ..Default::default()
        });
        assert_eq!(forward, ["l3", "l4"]);
        let older = texts(LogWindow {
            before: Some(stored[7].id),
            limit: Some(2),
            ..Default::default()
        });
        assert_eq!(older, ["l6", "l7"]);
        assert!(
            texts(LogWindow {
                after: Some(stored[9].id),
                ..Default::default()
            })
            .is_empty()
        );
        assert_eq!(texts(LogWindow::default()).len(), 10);
    }

    #[test]
    fn lines_survive_the_trash_and_leave_with_the_purged_issue() {
        let (db, _, issue) = fixture();
        let conn = db.write().unwrap();
        append(&conn, issue, "", &owned(&["one"])).unwrap();
        queries::delete_issue(&conn, issue).unwrap();
        assert_eq!(count(&conn, "SELECT count(*) FROM issue_log"), 1);
        conn.execute("DELETE FROM issues WHERE id = ?1", [issue])
            .unwrap();
        assert_eq!(count(&conn, "SELECT count(*) FROM issue_log"), 0);
    }
}
