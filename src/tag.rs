//! tag 注册、去重与 issue 关联。

use rusqlite::{Connection, OptionalExtension, params};

use crate::db;
use crate::error::Error;
use crate::models::Tag;

/// `--tag` 语法：`name` 或 `name:description`（冒号分隔）。
/// 逗号分隔多个 tag。
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
                // 含冒号但任一段为空（如 "a:" / ":desc"）→ 丢弃，不产出畸形 tag
                Some(_) => None,
                // 无冒号 → 纯 name
                None => Some((part.to_string(), None)),
            }
        })
        .collect()
}

/// 确保 tag 存在并返回其 id（新 tag 自动注册，已有则复用）。
pub fn ensure(conn: &Connection, name: &str, description: Option<&str>) -> Result<i64, Error> {
    if let Some(id) = query_id(conn, name)? {
        return Ok(id);
    }
    conn.execute(db::TAG_INSERT, params![name, description])?;
    query_id(conn, name)?
        .ok_or_else(|| Error::Other(format!("tag '{name}' just inserted but not found")))
}

/// 查询 tag id（不存在返回 None）。
pub fn query_id(conn: &Connection, name: &str) -> Result<Option<i64>, Error> {
    conn.query_row(db::TAG_SELECT_ID, params![name], |r| r.get(0))
        .optional()
        .map_err(Error::from)
}

/// 为 issue 关联多个 tag（幂等：重复关联忽略）。
pub fn attach(
    conn: &Connection,
    issue_id: i64,
    specs: &[(String, Option<String>)],
) -> Result<(), Error> {
    for (name, desc) in specs {
        let tag_id = ensure(conn, name, desc.as_deref())?;
        conn.execute(db::TAG_ATTACH, params![issue_id, tag_id])?;
    }
    Ok(())
}

/// 列出所有 tag（含关联 issue 数）。
pub fn list(conn: &Connection) -> Result<Vec<(Tag, i64)>, Error> {
    let mut stmt = conn.prepare(db::TAG_LIST)?;
    let rows = stmt.query_map([], |r| {
        let tag = Tag {
            id: r.get(0)?,
            name: r.get(1)?,
            description: r.get(2)?,
            created_at: r.get(3)?,
            updated_at: r.get(4)?,
        };
        let count: i64 = r.get(5)?;
        Ok((tag, count))
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Error::from)
}

/// 查询某 issue 的 tag 名列表（按 name 排序）。
pub fn names_for_issue(conn: &Connection, issue_id: i64) -> Result<Vec<String>, Error> {
    let mut stmt = conn.prepare(db::TAG_NAMES_FOR_ISSUE)?;
    let rows = stmt.query_map(params![issue_id], |r| r.get(0))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Error::from)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// 语法解析：name 与 name:desc 与逗号分隔。
    #[test]
    fn parse_specs_handles_name_and_desc() {
        let raw = vec!["storage,bug:缺陷".to_string(), "ui".to_string()];
        let specs = parse_specs(&raw);
        assert_eq!(
            specs,
            vec![
                ("storage".to_string(), None),
                ("bug".to_string(), Some("缺陷".to_string())),
                ("ui".to_string(), None),
            ]
        );
    }

    /// 边界：冒号段任一侧为空（"a:"/":desc"）不产出畸形 tag。
    #[test]
    fn parse_specs_drops_malformed_colon() {
        let raw = vec!["a:".to_string(), ":desc".to_string(), "ok".to_string()];
        let specs = parse_specs(&raw);
        assert_eq!(specs, vec![("ok".to_string(), None)]);
    }

    /// 新 tag 自动注册，重复 ensure 复用同一 id。
    #[test]
    fn ensure_registers_and_dedups() {
        let (conn, _) = setup();
        let id1 = ensure(&conn, "bug", Some("缺陷")).unwrap();
        let id2 = ensure(&conn, "bug", Some("缺陷")).unwrap();
        assert_eq!(id1, id2);
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM tags", [], |r| r.get(0))
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
            .query_row("SELECT COUNT(*) FROM issue_tags", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    /// 查询 issue 的 tag 名。
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
