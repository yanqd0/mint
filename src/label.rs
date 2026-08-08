//! label 注册、去重与 issue 关联。

use rusqlite::{Connection, OptionalExtension, params};

use crate::db;
use crate::error::Error;
use crate::models::Label;

/// `--label` 语法：`name` 或 `name:description`（冒号分隔）。
/// 逗号分隔多个 label。
pub fn parse_specs(raw: &[String]) -> Vec<(String, Option<String>)> {
    raw.iter()
        .flat_map(|s| s.split(','))
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .filter_map(|part| {
            match part.split_once(':') {
                // name:desc 两段均非空才采用
                Some((name, desc)) if !name.trim().is_empty() && !desc.trim().is_empty() => {
                    Some((name.trim().to_string(), Some(desc.trim().to_string())))
                }
                // 含冒号但任一段为空（如 "a:" / ":desc"）→ 丢弃，不产出畸形 label
                Some(_) => None,
                // 无冒号 → 纯 name
                None => Some((part.to_string(), None)),
            }
        })
        .collect()
}

/// 确保 label 存在并返回其 id（新 label 自动注册，已有则复用）。
pub fn ensure(conn: &Connection, name: &str, description: Option<&str>) -> Result<i64, Error> {
    if let Some(id) = query_id(conn, name)? {
        return Ok(id);
    }
    conn.execute(db::LABEL_INSERT, params![name, description])?;
    query_id(conn, name)?
        .ok_or_else(|| Error::Other(format!("label '{name}' just inserted but not found")))
}

/// 查询 label id（不存在返回 None）。
pub fn query_id(conn: &Connection, name: &str) -> Result<Option<i64>, Error> {
    conn.query_row(db::LABEL_SELECT_ID, params![name], |r| r.get(0))
        .optional()
        .map_err(Error::from)
}

/// 为 issue 关联多个 label（幂等：重复关联忽略）。
pub fn attach(
    conn: &Connection,
    issue_id: i64,
    specs: &[(String, Option<String>)],
) -> Result<(), Error> {
    for (name, desc) in specs {
        let label_id = ensure(conn, name, desc.as_deref())?;
        conn.execute(db::LABEL_ATTACH, params![issue_id, label_id])?;
    }
    Ok(())
}

/// 列出所有 label（含关联 issue 数）。
pub fn list(conn: &Connection) -> Result<Vec<(Label, i64)>, Error> {
    let mut stmt = conn.prepare(db::LABEL_LIST)?;
    let rows = stmt.query_map([], |r| {
        let label = Label {
            id: r.get(0)?,
            name: r.get(1)?,
            description: r.get(2)?,
            created_at: r.get(3)?,
            updated_at: r.get(4)?,
        };
        let count: i64 = r.get(5)?;
        Ok((label, count))
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Error::from)
}

/// 删除 label（按 name 查找 id），同步清关联 issue_labels 行，事务内原子提交。
pub fn delete(conn: &Connection, name: &str) -> Result<(), Error> {
    let id =
        query_id(conn, name)?.ok_or_else(|| Error::Other(format!("label '{name}' not found")))?;
    crate::container::delete_txn(conn, db::LABEL_DELETE, id, |_| Ok(()))
}

/// 查询某 issue 的 label 名列表（按 name 排序）。
pub fn names_for_issue(conn: &Connection, issue_id: i64) -> Result<Vec<String>, Error> {
    let mut stmt = conn.prepare(db::LABEL_NAMES_FOR_ISSUE)?;
    let rows = stmt.query_map(params![issue_id], |r| r.get(0))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Error::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn setup() -> (Connection, i64) {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::migrate_for_test(&conn);
        conn.execute("INSERT INTO projects (name) VALUES ('p')", [])
            .unwrap();
        let pid: i64 = conn
            .query_row("SELECT id FROM projects WHERE name='p'", [], |r| r.get(0))
            .unwrap();
        conn.execute(
            "INSERT INTO issues (title, project_id) VALUES ('issue', ?1)",
            params![pid],
        )
        .unwrap();
        let iid: i64 = conn
            .query_row("SELECT id FROM issues", [], |r| r.get(0))
            .unwrap();
        (conn, iid)
    }

    /// 语法解析参数化：name / name:desc / 逗号分隔 / 畸形冒号段 / 空输入。
    #[rstest]
    #[case::name_only(vec!["ui".to_string()], vec![("ui".to_string(), None)])]
    #[case::name_with_desc(vec!["bug:缺陷".to_string()], vec![("bug".to_string(), Some("缺陷".to_string()))])]
    #[case::multiple(vec!["storage".to_string(), "bug:缺陷".to_string(), "ui".to_string()],
        vec![
            ("storage".to_string(), None),
            ("bug".to_string(), Some("缺陷".to_string())),
            ("ui".to_string(), None),
        ])]
    #[case::malformed_colon(vec!["a:".to_string(), ":desc".to_string(), "ok".to_string()], vec![("ok".to_string(), None)])]
    #[case::empty(vec![], vec![])]
    fn parse_specs_cases(
        #[case] raw: Vec<String>,
        #[case] expected: Vec<(String, Option<String>)>,
    ) {
        assert_eq!(parse_specs(&raw), expected);
    }

    /// 新 label 自动注册，重复 ensure 复用同一 id。
    #[test]
    fn ensure_registers_and_dedups() {
        let (conn, _) = setup();
        let id1 = ensure(&conn, "bug", Some("缺陷")).unwrap();
        let id2 = ensure(&conn, "bug", Some("缺陷")).unwrap();
        assert_eq!(id1, id2);
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM labels", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    /// attach 幂等：重复 attach 不重复插关联。
    #[test]
    fn attach_is_idempotent() {
        let (conn, iid) = setup();
        let specs = vec![("bug".to_string(), Some("缺陷".to_string()))];
        attach(&conn, iid, &specs).unwrap();
        attach(&conn, iid, &specs).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM issue_labels", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    /// delete 删除 label 及其 issue 关联，关联标签消失。
    #[test]
    fn delete_removes_label_and_links() {
        let (conn, iid) = setup();
        ensure(&conn, "bug", Some("缺陷")).unwrap();
        attach(&conn, iid, &[("bug".to_string(), None)]).unwrap();
        delete(&conn, "bug").unwrap();
        // label 行已删除
        let cnt: i64 = conn
            .query_row("SELECT COUNT(*) FROM labels WHERE name='bug'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(cnt, 0);
        // 关联行已清
        let ic: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM issue_labels WHERE issue_id = ?1",
                params![iid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(ic, 0);
    }

    /// delete 不存在的 label 报 not found。
    #[test]
    fn delete_missing_errors() {
        let (conn, _) = setup();
        let err = delete(&conn, "nosuch").unwrap_err();
        assert!(
            err.to_string().contains("label 'nosuch' not found"),
            "{err}"
        );
    }

    /// 查询 issue 的 label 名。
    #[test]
    fn names_for_issue_returns_sorted() {
        let (conn, iid) = setup();
        attach(
            &conn,
            iid,
            &[("bug".to_string(), None), ("storage".to_string(), None)],
        )
        .unwrap();
        let names = names_for_issue(&conn, iid).unwrap();
        assert_eq!(names, vec!["bug", "storage"]);
    }
}
