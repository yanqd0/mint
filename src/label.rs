//! label 注册、去重与 issue 关联。

use std::collections::HashMap;

use rusqlite::{Connection, OptionalExtension, params};

use crate::db;
use crate::error::Error;
use crate::models::Label;

/// `--label` 语法：`name` / `name:desc` / `name:desc:color`（冒号分隔，逗号分隔多个）。
/// 任一段空（如 "a:" / "a:b:"）→ 丢弃，不产出畸形 label。
pub fn parse_specs(raw: &[String]) -> Vec<(String, Option<String>, Option<String>)> {
    raw.iter()
        .flat_map(|s| s.split(','))
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .filter_map(|part| {
            let segs: Vec<&str> = part.split(':').map(str::trim).collect();
            match segs.as_slice() {
                [name] if !name.is_empty() => Some(((*name).to_string(), None, None)),
                [name, desc] if !name.is_empty() && !desc.is_empty() => {
                    Some(((*name).to_string(), Some((*desc).to_string()), None))
                }
                [name, desc, color]
                    if !name.is_empty() && !desc.is_empty() && !color.is_empty() =>
                {
                    Some((
                        (*name).to_string(),
                        Some((*desc).to_string()),
                        Some((*color).to_string()),
                    ))
                }
                _ => None,
            }
        })
        .collect()
}

/// 确保 label 存在并返回其 id（新 label 自动注册，已有则复用）。
/// color 显式指定优先；未指定则自动配色（next_color 基于既有颜色，色差大）。
pub fn ensure(
    conn: &Connection,
    name: &str,
    description: Option<&str>,
    color: Option<&str>,
) -> Result<i64, Error> {
    if let Some(id) = query_id(conn, name)? {
        return Ok(id);
    }
    let color = match color {
        Some(c) => c.to_string(),
        None => next_color(&existing_colors(conn)?),
    };
    conn.execute(db::LABEL_INSERT, params![name, description, color])?;
    query_id(conn, name)?
        .ok_or_else(|| Error::Other(format!("label '{name}' just inserted but not found")))
}

/// 查询既有 label 颜色（自动配色用）。
fn existing_colors(conn: &Connection) -> Result<Vec<String>, Error> {
    let mut stmt = conn.prepare(db::LABEL_COLORS)?;
    let rows = stmt.query_map([], |r| r.get(0))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Error::from)
}

/// 查询 label id（不存在返回 None）。
pub fn query_id(conn: &Connection, name: &str) -> Result<Option<i64>, Error> {
    conn.query_row(db::LABEL_SELECT_ID, params![name], |r| r.get(0))
        .optional()
        .map_err(Error::from)
}

/// 为 issue 关联多个 label（幂等：重复关联忽略；label 不存在自动注册 + 配色）。
pub fn attach(
    conn: &Connection,
    issue_id: i64,
    specs: &[(String, Option<String>, Option<String>)],
) -> Result<(), Error> {
    for (name, desc, color) in specs {
        let label_id = ensure(conn, name, desc.as_deref(), color.as_deref())?;
        conn.execute(db::LABEL_ATTACH, params![issue_id, label_id])?;
    }
    Ok(())
}

/// 更新 label 本体（color/description，COALESCE 保留未提供字段）。
pub fn set(
    conn: &Connection,
    name: &str,
    color: Option<&str>,
    description: Option<&str>,
) -> Result<(), Error> {
    let id =
        query_id(conn, name)?.ok_or_else(|| Error::Other(format!("label '{name}' not found")))?;
    conn.execute(db::LABEL_UPDATE, params![color, description, id])?;
    Ok(())
}

/// 自动配色：候选为 HSL golden-angle 调色板（24 色，色相环均匀分布），
/// 取与既有颜色 RGB 欧氏距离最小距离最大的候选（最大最小贪心）。首次返回调色板第 1 色。
pub fn next_color(existing: &[String]) -> String {
    let palette = palette();
    if existing.is_empty() {
        return palette[0].clone();
    }
    let existing_rgb: Vec<(f64, f64, f64)> = existing.iter().filter_map(|c| parse_hex(c)).collect();
    let mut best = palette[0].clone();
    let mut best_min = -1.0f64;
    for cand in &palette {
        if let Some(cr) = parse_hex(cand) {
            let min_dist = existing_rgb
                .iter()
                .map(|&(r, g, b)| {
                    let dr = cr.0 - r;
                    let dg = cr.1 - g;
                    let db = cr.2 - b;
                    dr * dr + dg * dg + db * db
                })
                .fold(f64::INFINITY, f64::min);
            if min_dist > best_min {
                best_min = min_dist;
                best = cand.clone();
            }
        }
    }
    best
}

/// HSL golden-angle 调色板（24 色，hue 递增 golden angle 137.5°，色相环均匀分布）。
fn palette() -> Vec<String> {
    (0..24)
        .map(|i| {
            let hue = (i as f64 * 137.508) % 360.0;
            let (r, g, b) = hsl_to_rgb(hue, 0.7, 0.55);
            format!("#{r:02x}{g:02x}{b:02x}")
        })
        .collect()
}

/// HSL → RGB（h ∈ [0,360)，s/l ∈ [0,1]，返回 0-255）。
fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r1, g1, b1) = if hp < 1.0 {
        (c, x, 0.0)
    } else if hp < 2.0 {
        (x, c, 0.0)
    } else if hp < 3.0 {
        (0.0, c, x)
    } else if hp < 4.0 {
        (0.0, x, c)
    } else if hp < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    let m = l - c / 2.0;
    (
        ((r1 + m) * 255.0).round() as u8,
        ((g1 + m) * 255.0).round() as u8,
        ((b1 + m) * 255.0).round() as u8,
    )
}

/// 解析 hex（`#rrggbb` 或 `rrggbb`）为 RGB 浮点。
fn parse_hex(hex: &str) -> Option<(f64, f64, f64)> {
    let h = hex.trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&h[0..2], 16).ok()? as f64;
    let g = u8::from_str_radix(&h[2..4], 16).ok()? as f64;
    let b = u8::from_str_radix(&h[4..6], 16).ok()? as f64;
    Some((r, g, b))
}

/// 从 issue 摘除多个 label 关联（幂等：不存在的 label / 无关联跳过；不删 label 本体）。
/// 返回实际解除的关联数。
pub fn detach(conn: &Connection, issue_id: i64, names: &[&str]) -> Result<usize, Error> {
    let mut detached = 0;
    for name in names {
        if let Some(label_id) = query_id(conn, name)? {
            detached += conn.execute(db::ISSUE_LABELS_DELETE, params![issue_id, label_id])?;
        }
    }
    Ok(detached)
}

/// 列出所有 label（含关联 issue 数）。
pub fn list(conn: &Connection) -> Result<Vec<(Label, i64)>, Error> {
    let mut stmt = conn.prepare(db::LABEL_LIST)?;
    let rows = stmt.query_map([], |r| {
        let label = Label {
            id: r.get(0)?,
            name: r.get(1)?,
            description: r.get(2)?,
            color: r.get(3)?,
            created_at: r.get(4)?,
            updated_at: r.get(5)?,
        };
        let count: i64 = r.get(6)?;
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

/// 批量取全部 issue 的 label 名（一次查询，替代逐 issue `names_for_issue`；dashboard 全量加载用）。
/// 每 issue 按 name 排序（与 `LABEL_NAMES_FOR_ISSUE` 的 `ORDER BY t.name` 一致）。
pub fn names_for_issues(conn: &Connection) -> Result<HashMap<i64, Vec<String>>, Error> {
    let mut stmt = conn.prepare(db::ISSUE_LABELS_FOR_ALL)?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
    let mut out: HashMap<i64, Vec<String>> = HashMap::new();
    for row in rows {
        let (issue_id, name) = row?;
        out.entry(issue_id).or_default().push(name);
    }
    for names in out.values_mut() {
        names.sort();
    }
    Ok(out)
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
    #[case::name_only(vec!["ui".to_string()], vec![("ui".to_string(), None, None)])]
    #[case::name_with_desc(vec!["bug:缺陷".to_string()], vec![("bug".to_string(), Some("缺陷".to_string()), None)])]
    #[case::name_desc_color(vec!["bug:缺陷:#d73a4a".to_string()], vec![("bug".to_string(), Some("缺陷".to_string()), Some("#d73a4a".to_string()))])]
    #[case::multiple(vec!["storage".to_string(), "bug:缺陷".to_string(), "ui".to_string()],
        vec![
            ("storage".to_string(), None, None),
            ("bug".to_string(), Some("缺陷".to_string()), None),
            ("ui".to_string(), None, None),
        ])]
    #[case::malformed_colon(vec!["a:".to_string(), ":desc".to_string(), "ok".to_string()], vec![("ok".to_string(), None, None)])]
    #[case::empty(vec![], vec![])]
    fn parse_specs_cases(
        #[case] raw: Vec<String>,
        #[case] expected: Vec<(String, Option<String>, Option<String>)>,
    ) {
        assert_eq!(parse_specs(&raw), expected);
    }

    /// 新 label 自动注册，重复 ensure 复用同一 id。
    #[test]
    fn ensure_registers_and_dedups() {
        let (conn, _) = setup();
        let id1 = ensure(&conn, "bug", Some("缺陷"), None).unwrap();
        let id2 = ensure(&conn, "bug", Some("缺陷"), None).unwrap();
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
        let specs = vec![("bug".to_string(), Some("缺陷".to_string()), None)];
        attach(&conn, iid, &specs).unwrap();
        attach(&conn, iid, &specs).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM issue_labels", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    /// detach 从 issue 摘除 label 关联（label 本体保留）。
    #[test]
    fn detach_removes_links_keeps_label() {
        let (conn, iid) = setup();
        attach(
            &conn,
            iid,
            &[
                ("bug".to_string(), None, None),
                ("storage".to_string(), None, None),
            ],
        )
        .unwrap();
        detach(&conn, iid, &["bug"]).unwrap();
        let names = names_for_issue(&conn, iid).unwrap();
        assert_eq!(names, vec!["storage"]);
        // label 本体仍在（detach 只摘关联，不删 label）
        let cnt: i64 = conn
            .query_row("SELECT COUNT(*) FROM labels WHERE name='bug'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(cnt, 1);
    }

    /// detach 幂等：重复 detach / 不存在 label / 无关联均无副作用。
    #[test]
    fn detach_idempotent_and_missing() {
        let (conn, iid) = setup();
        attach(&conn, iid, &[("bug".to_string(), None, None)]).unwrap();
        detach(&conn, iid, &["bug", "bug", "nosuch"]).unwrap();
        let names = names_for_issue(&conn, iid).unwrap();
        assert!(names.is_empty(), "应无剩余 label: {names:?}");
        let cnt: i64 = conn
            .query_row("SELECT COUNT(*) FROM labels WHERE name='bug'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(cnt, 1, "label 本体不应被删除");
    }

    /// delete 删除 label 及其 issue 关联，关联标签消失。
    #[test]
    fn delete_removes_label_and_links() {
        let (conn, iid) = setup();
        ensure(&conn, "bug", Some("缺陷"), None).unwrap();
        attach(&conn, iid, &[("bug".to_string(), None, None)]).unwrap();
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
            &[
                ("bug".to_string(), None, None),
                ("storage".to_string(), None, None),
            ],
        )
        .unwrap();
        let names = names_for_issue(&conn, iid).unwrap();
        assert_eq!(names, vec!["bug", "storage"]);
    }

    /// names_for_issues 批量结果与逐 issue names_for_issue 一致（按 name 排序）。
    #[test]
    fn names_for_issues_matches_single() {
        let (conn, iid) = setup();
        attach(
            &conn,
            iid,
            &[
                ("storage".to_string(), None, None),
                ("bug".to_string(), None, None),
            ],
        )
        .unwrap();
        let map = names_for_issues(&conn).unwrap();
        assert_eq!(
            map.get(&iid).unwrap(),
            &names_for_issue(&conn, iid).unwrap()
        );
    }

    /// 自动配色：首次返回调色板首色，连续创建不同色（色差大）。
    #[test]
    fn ensure_auto_colors_distinct() {
        let (conn, _) = setup();
        ensure(&conn, "a", None, None).unwrap();
        ensure(&conn, "b", None, None).unwrap();
        let colors: Vec<String> = conn
            .prepare("SELECT color FROM labels ORDER BY id")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(colors.len(), 2, "两 label 都应配色");
        assert_ne!(colors[0], colors[1], "连续创建色应不同");
    }

    /// next_color：无既有返回调色板首色；新增色与既有颜色最小距离最大化。
    #[test]
    fn next_color_maximizes_min_distance() {
        let first = next_color(&[]);
        assert_eq!(first, palette()[0]);
        let c = next_color(&["#ff0000".to_string(), "#00ff00".to_string()]);
        assert!(c != "#ff0000" && c != "#00ff00", "新色应避开既有: {c}");
    }

    /// set 更新 color/description（COALESCE 保留未提供字段）。
    #[test]
    fn set_updates_color_and_description() {
        let (conn, _) = setup();
        ensure(&conn, "bug", Some("缺陷"), Some("#d73a4a")).unwrap();
        set(&conn, "bug", Some("#ff0000"), None).unwrap();
        let (color, desc): (Option<String>, Option<String>) = conn
            .query_row(
                "SELECT color, description FROM labels WHERE name='bug'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(color.as_deref(), Some("#ff0000"));
        assert_eq!(desc.as_deref(), Some("缺陷"));
    }
}
