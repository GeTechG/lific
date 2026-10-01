//! Free-form text properties on issues: per-issue `name -> value` pairs with
//! no per-project schema (migration 059).
//!
//! Writes are deltas (`set` / `unset`), applied inside the issue's own
//! create/update savepoint, so a caller never has to read the current set to
//! change one entry and two writers touching different names cannot undo
//! each other.

use std::collections::{BTreeMap, HashMap};

use rusqlite::{Connection, params};

use crate::error::LificError;

/// Longest property name, in bytes (names are ASCII).
pub const MAX_PROPERTY_NAME_BYTES: usize = 64;

/// Largest property value, in bytes of UTF-8. A property is a short piece of
/// metadata; anything longer belongs in the description or a page.
pub const MAX_PROPERTY_VALUE_BYTES: usize = 4096;

/// Most properties one issue carries.
pub const MAX_PROPERTIES_PER_ISSUE: i64 = 64;

/// `[a-z0-9][a-z0-9_-]{0,63}`.
pub fn validate_property_name(name: &str) -> Result<(), LificError> {
    let allowed = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-';
    let valid = !name.is_empty()
        && name.len() <= MAX_PROPERTY_NAME_BYTES
        && name.bytes().all(allowed)
        && !matches!(name.as_bytes()[0], b'_' | b'-');
    if valid {
        Ok(())
    } else {
        Err(LificError::BadRequest(format!(
            "invalid property name '{name}'. Use 1 to {MAX_PROPERTY_NAME_BYTES} characters from \
             a-z, 0-9, '_' and '-', starting with a letter or digit."
        )))
    }
}

pub fn validate_property_value(name: &str, value: &str) -> Result<(), LificError> {
    if value.is_empty() {
        return Err(LificError::BadRequest(format!(
            "property '{name}' cannot be set to an empty value; unset it instead"
        )));
    }
    if value.len() > MAX_PROPERTY_VALUE_BYTES {
        return Err(LificError::BadRequest(format!(
            "property '{name}' is {} bytes; the limit is {MAX_PROPERTY_VALUE_BYTES}",
            value.len()
        )));
    }
    Ok(())
}

/// Check a whole delta before anything is written.
pub fn validate_delta(set: &BTreeMap<String, String>, unset: &[String]) -> Result<(), LificError> {
    for (name, value) in set {
        validate_property_name(name)?;
        validate_property_value(name, value)?;
    }
    for name in unset {
        validate_property_name(name)?;
        if set.contains_key(name) {
            return Err(LificError::BadRequest(format!(
                "property '{name}' is both set and unset in the same request"
            )));
        }
    }
    Ok(())
}

/// Apply a validated delta to one issue. Setting a name to the value it
/// already has, and unsetting a name the issue does not carry, change nothing
/// (no audit row, no seq bump). The caller owns the surrounding savepoint.
pub fn apply_delta(
    conn: &Connection,
    issue_id: i64,
    set: &BTreeMap<String, String>,
    unset: &[String],
) -> Result<(), LificError> {
    if set.is_empty() && unset.is_empty() {
        return Ok(());
    }
    for name in unset {
        conn.execute(
            "DELETE FROM issue_properties WHERE issue_id = ?1 AND name = ?2",
            params![issue_id, name],
        )?;
    }
    for (name, value) in set {
        conn.execute(
            "INSERT INTO issue_properties (issue_id, name, value) VALUES (?1, ?2, ?3)
             ON CONFLICT (issue_id, name) DO UPDATE
                SET value = excluded.value, updated_at = datetime('now')
              WHERE issue_properties.value IS NOT excluded.value",
            params![issue_id, name, value],
        )?;
    }
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM issue_properties WHERE issue_id = ?1",
        params![issue_id],
        |row| row.get(0),
    )?;
    if count > MAX_PROPERTIES_PER_ISSUE {
        return Err(LificError::BadRequest(format!(
            "an issue carries at most {MAX_PROPERTIES_PER_ISSUE} properties"
        )));
    }
    Ok(())
}

/// One issue's properties, by name.
pub fn list_properties(
    conn: &Connection,
    issue_id: i64,
) -> Result<BTreeMap<String, String>, LificError> {
    Ok(properties_by_issue(conn, &[issue_id])?
        .remove(&issue_id)
        .unwrap_or_default())
}

/// Properties per issue id, in one round trip (the list and `/changes` pages).
pub fn properties_by_issue(
    conn: &Connection,
    ids: &[i64],
) -> Result<HashMap<i64, BTreeMap<String, String>>, LificError> {
    let mut by_issue: HashMap<i64, BTreeMap<String, String>> = HashMap::new();
    if ids.is_empty() {
        return Ok(by_issue);
    }
    let placeholders: String = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let mut stmt = conn.prepare(&format!(
        "SELECT issue_id, name, value FROM issue_properties WHERE issue_id IN ({placeholders})"
    ))?;
    let rows = stmt.query_map(rusqlite::params_from_iter(ids), |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    for row in rows {
        let (issue_id, name, value) = row?;
        by_issue.entry(issue_id).or_default().insert(name, value);
    }
    Ok(by_issue)
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
                    name: "Props".into(),
                    identifier: "PRP".into(),
                    ..Default::default()
                },
            )
            .unwrap()
            .id;
            let issue = queries::create_issue(
                &conn,
                &CreateIssue {
                    project_id: project,
                    title: "Carrier".into(),
                    set_properties: map(&[("footprint", "src/a.js, test/")]),
                    ..Default::default()
                },
            )
            .unwrap()
            .id;
            (project, issue)
        };
        (db, project, issue)
    }

    fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
            .collect()
    }

    fn delta(set: &[(&str, &str)], unset: &[&str]) -> UpdateIssue {
        UpdateIssue {
            set_properties: map(set),
            unset_properties: unset.iter().map(|name| (*name).to_string()).collect(),
            ..Default::default()
        }
    }

    /// `(field, old, new)` of every property audit row on the issue, oldest first.
    fn audit(conn: &Connection, issue: i64) -> Vec<(String, Option<String>, Option<String>)> {
        conn.prepare(
            "SELECT field, old_value, new_value FROM audit_log
              WHERE entity_type = 'issue' AND entity_id = ?1 AND action = 'update'
                AND field LIKE 'property:%' ORDER BY id",
        )
        .unwrap()
        .query_map([issue], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
    }

    #[test]
    fn create_stores_properties_and_reads_return_them() {
        let (db, project, issue) = fixture();
        let conn = db.read().unwrap();
        let expected = map(&[("footprint", "src/a.js, test/")]);
        assert_eq!(
            queries::get_issue(&conn, issue).unwrap().properties,
            expected
        );
        let listed = queries::list_issues(
            &conn,
            &ListIssuesQuery {
                project_id: Some(project),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(listed[0].properties, expected);
    }

    #[test]
    fn an_issue_without_properties_serializes_an_empty_object() {
        let (db, project, _) = fixture();
        let conn = db.write().unwrap();
        let bare = queries::create_issue(
            &conn,
            &CreateIssue {
                project_id: project,
                title: "Bare".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let json = serde_json::to_value(&bare).unwrap();
        assert_eq!(json["properties"], serde_json::json!({}));
    }

    #[test]
    fn an_update_is_a_delta_that_leaves_other_properties_alone() {
        let (db, _, issue) = fixture();
        let conn = db.write().unwrap();
        queries::update_issue(&conn, issue, &delta(&[("pr", "41")], &[])).unwrap();
        let updated = queries::update_issue(
            &conn,
            issue,
            &delta(&[("footprint", "src/b.js")], &["pr", "absent"]),
        )
        .unwrap();
        assert_eq!(updated.properties, map(&[("footprint", "src/b.js")]));
    }

    #[test]
    fn each_changed_property_writes_one_audit_row_and_a_no_op_writes_none() {
        let (db, _, issue) = fixture();
        let conn = db.write().unwrap();
        let before = queries::get_issue(&conn, issue).unwrap().seq;
        // Same value again, and unsetting what is not there: nothing happens.
        let same = queries::update_issue(
            &conn,
            issue,
            &delta(&[("footprint", "src/a.js, test/")], &["absent"]),
        )
        .unwrap();
        assert_eq!(same.seq, before);

        let changed = queries::update_issue(
            &conn,
            issue,
            &delta(&[("footprint", "src/b.js"), ("pr", "41")], &[]),
        )
        .unwrap();
        assert!(changed.seq > before, "a property change advances the seq");
        queries::update_issue(&conn, issue, &delta(&[], &["pr"])).unwrap();

        let some = |value: &str| Some(value.to_string());
        assert_eq!(
            audit(&conn, issue),
            [
                (
                    "property:footprint".to_string(),
                    None,
                    some("src/a.js, test/")
                ),
                (
                    "property:footprint".to_string(),
                    some("src/a.js, test/"),
                    some("src/b.js")
                ),
                ("property:pr".to_string(), None, some("41")),
                ("property:pr".to_string(), some("41"), None),
            ]
        );
    }

    #[test]
    fn invalid_names_and_values_are_refused_and_nothing_is_written() {
        let (db, _, issue) = fixture();
        let conn = db.write().unwrap();
        let too_long = "x".repeat(MAX_PROPERTY_VALUE_BYTES + 1);
        let long_name = "n".repeat(MAX_PROPERTY_NAME_BYTES + 1);
        for (name, value) in [
            ("Footprint", "v"),
            ("-lead", "v"),
            ("_lead", "v"),
            ("has space", "v"),
            ("", "v"),
            (long_name.as_str(), "v"),
            ("empty", ""),
            ("big", too_long.as_str()),
        ] {
            // A valid title edit rides along: the refusal must take it down too.
            let mut input = delta(&[(name, value)], &[]);
            input.title = Some("Should not land".into());
            let error = queries::update_issue(&conn, issue, &input).unwrap_err();
            assert!(
                matches!(error, LificError::BadRequest(_)),
                "{name:?}={value:?}: {error}"
            );
        }
        assert!(matches!(
            queries::update_issue(&conn, issue, &delta(&[], &["Bad Name"])).unwrap_err(),
            LificError::BadRequest(_)
        ));
        assert!(matches!(
            queries::update_issue(&conn, issue, &delta(&[("pr", "1")], &["pr"])).unwrap_err(),
            LificError::BadRequest(_)
        ));
        let issue = queries::get_issue(&conn, issue).unwrap();
        assert_eq!(issue.title, "Carrier");
        assert_eq!(issue.properties, map(&[("footprint", "src/a.js, test/")]));

        // The limits themselves are accepted.
        let edge_name = "n".repeat(MAX_PROPERTY_NAME_BYTES);
        let edge_value = "é".repeat(MAX_PROPERTY_VALUE_BYTES / 2);
        queries::update_issue(
            &conn,
            issue.id,
            &delta(
                &[(edge_name.as_str(), edge_value.as_str()), ("0-a_b", "ok")],
                &[],
            ),
        )
        .unwrap();
    }

    #[test]
    fn an_issue_holds_a_bounded_number_of_properties() {
        let (db, _, issue) = fixture();
        let conn = db.write().unwrap();
        let names: Vec<String> = (0..MAX_PROPERTIES_PER_ISSUE)
            .map(|index| format!("p{index}"))
            .collect();
        let pairs: Vec<(&str, &str)> = names.iter().map(|name| (name.as_str(), "v")).collect();
        // The fixture's `footprint` plus these is one too many.
        let error = queries::update_issue(&conn, issue, &delta(&pairs, &[])).unwrap_err();
        assert!(matches!(error, LificError::BadRequest(_)), "{error}");
        assert_eq!(list_properties(&conn, issue).unwrap().len(), 1);
        queries::update_issue(&conn, issue, &delta(&pairs, &["footprint"])).unwrap();
    }

    #[test]
    fn a_stale_expected_seq_refuses_the_delta_but_a_bare_delta_needs_no_read() {
        let (db, _, issue) = fixture();
        let conn = db.write().unwrap();
        let mut stale = delta(&[("pr", "41")], &["footprint"]);
        stale.expected_seq = Some(0);
        assert!(matches!(
            queries::update_issue(&conn, issue, &stale).unwrap_err(),
            LificError::UpdateConflict { .. }
        ));
        assert_eq!(
            list_properties(&conn, issue).unwrap(),
            map(&[("footprint", "src/a.js, test/")])
        );
        queries::update_issue(&conn, issue, &delta(&[("pr", "41")], &[])).unwrap();
    }

    #[test]
    fn properties_survive_trash_and_restore_and_go_with_a_purge() {
        let (db, _, issue) = fixture();
        let conn = db.write().unwrap();
        queries::delete_issue(&conn, issue).unwrap();
        let restored = queries::restore_issue(&conn, issue).unwrap();
        assert_eq!(
            restored.properties,
            map(&[("footprint", "src/a.js, test/")])
        );

        queries::delete_issue(&conn, issue).unwrap();
        conn.execute(
            "UPDATE issues SET deleted_at = '2000-01-01 00:00:00' WHERE id = ?1",
            [issue],
        )
        .unwrap();
        let rows_before = audit(&conn, issue).len();
        queries::trash::purge_tombstones(&conn, 30).unwrap();
        let left: i64 = conn
            .query_row("SELECT COUNT(*) FROM issue_properties", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(left, 0);
        assert_eq!(
            audit(&conn, issue).len(),
            rows_before,
            "the purge's cascade is not audited as an unset"
        );
    }
}
