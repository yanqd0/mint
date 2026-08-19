//! 同步快照导入（git+SQL 路线，plan #84 #367）。
//!
//! `import_sql`：把确定性 SQL 快照**幂等合并**进本机库——业务键幂等（projects/labels/
//! milestones 按 UNIQUE 键、plans 按 title+milestone_id）、issues 按 uid LWW（updated_at
//! 取新）、id 冲突重映射（uid 是稳定跨机键，本地 id 可重排）并修正全部引用。

use std::collections::HashMap;

use rusqlite::{Connection, OptionalExtension, params_from_iter, types::Value};

use crate::error::Error;

/// 合并结果统计。
#[derive(Debug, Default, Clone, Copy)]
pub struct MergeReport {
    pub inserted: usize,
    pub updated: usize,
    pub skipped: usize,
}

/// 把 SQL 快照幂等合并进目标库（整事务；临时库用 std 临时文件，零新增依赖）。
pub fn import_sql(conn: &mut Connection, sql: &str) -> Result<MergeReport, Error> {
    let path = temp_db_path()?;
    let res = import_inner(conn, sql, &path);
    let _ = std::fs::remove_file(&path);
    res
}

fn import_inner(conn: &mut Connection, sql: &str, path: &std::path::Path) -> Result<MergeReport, Error> {
    // 快照重放到独立临时库（标准 SQL 直接 execute_batch）。
    let tmp = Connection::open(path)?;
    tmp.execute_batch(sql)?;
    tmp.execute_batch("PRAGMA foreign_keys = ON")?;

    conn.execute_batch(&format!("ATTACH DATABASE '{}' AS tmp", path.display()))?;
    let res = merge_all(conn, &tmp);
    conn.execute_batch("DETACH DATABASE tmp").ok();
    res
}

fn merge_all(conn: &Connection, tmp: &Connection) -> Result<MergeReport, Error> {
    let mut report = MergeReport::default();
    let mut projects_map = HashMap::new();
    let mut labels_map = HashMap::new();
    let mut milestones_map = HashMap::new();
    let mut plans_map = HashMap::new();
    let mut issues_map = HashMap::new();

    conn.execute_batch("BEGIN IMMEDIATE")?;

    merge_machines(conn, tmp, &mut report)?;
    merge_keyed(conn, tmp, "projects", "name", &mut projects_map, &mut report)?;
    merge_keyed(conn, tmp, "labels", "name", &mut labels_map, &mut report)?;
    merge_keyed(conn, tmp, "milestones", "version", &mut milestones_map, &mut report)?;
    merge_plans(conn, tmp, &milestones_map, &mut plans_map, &mut report)?;
    merge_issues(conn, tmp, &projects_map, &plans_map, &mut issues_map, &mut report)?;
    merge_assoc(conn, tmp, "issue_labels", &["issue_id", "label_id"], &issues_map, &labels_map, &mut report)?;
    merge_assoc(conn, tmp, "issue_links", &["from_id", "to_id"], &issues_map, &issues_map, &mut report)?;
    merge_assoc(conn, tmp, "milestone_direct_issues", &["milestone_id", "issue_id"], &milestones_map, &issues_map, &mut report)?;

    conn.execute_batch("COMMIT")?;
    Ok(report)
}

// ── 表专用合并 ──────────────────────────────────────────────

/// machines：PK=machine_id（文本），存在即跳过。
fn merge_machines(conn: &Connection, tmp: &Connection, report: &mut MergeReport) -> Result<(), Error> {
    let cols = columns(tmp, "machines")?;
    for row in read_rows(tmp, "machines", &cols)? {
        let key = &row[col_idx(&cols, "machine_id")];
        let exists = conn
            .query_row("SELECT 1 FROM machines WHERE machine_id = ?1", [key], |_| Ok(()))
            .optional()?
            .is_some();
        if exists {
            report.skipped += 1;
        } else {
            insert_row(conn, "machines", &cols, &row)?;
            report.inserted += 1;
        }
    }
    Ok(())
}

/// 业务键幂等表（projects/labels/milestones）：UNIQUE 键存在则跳过，否则插入并建 id 映射。
fn merge_keyed(
    conn: &Connection,
    tmp: &Connection,
    table: &str,
    key_col: &str,
    id_map: &mut HashMap<i64, i64>,
    report: &mut MergeReport,
) -> Result<(), Error> {
    let cols = columns(tmp, table)?;
    for mut row in read_rows(tmp, table, &cols)? {
        let orig_id = row_id(&row, &cols);
        let key = row[col_idx(&cols, key_col)].clone();
        let sql = format!("SELECT id FROM {table} WHERE {key_col} = ?1");
        let existing: Option<i64> = conn.query_row(&sql, [key], |r| r.get(0)).optional()?;
        let target_id = match existing {
            Some(id) => {
                report.skipped += 1;
                id
            }
            None => {
                let new_id = if let Some(orig) = orig_id {
                    if id_taken(conn, table, orig)? {
                        next_id(conn, table)?
                    } else {
                        orig
                    }
                } else {
                    next_id(conn, table)?
                };
                set_id(&mut row, &cols, new_id);
                insert_row(conn, table, &cols, &row)?;
                report.inserted += 1;
                new_id
            }
        };
        if let Some(orig) = orig_id {
            id_map.insert(orig, target_id);
        }
    }
    Ok(())
}

/// plans：幂等键 (title, milestone_id)（milestone_id 经 milestones 映射）；无 UNIQUE，用 EXISTS 判定。
fn merge_plans(
    conn: &Connection,
    tmp: &Connection,
    milestones_map: &HashMap<i64, i64>,
    id_map: &mut HashMap<i64, i64>,
    report: &mut MergeReport,
) -> Result<(), Error> {
    let cols = columns(tmp, "plans")?;
    for mut row in read_rows(tmp, "plans", &cols)? {
        let orig_id = row_id(&row, &cols);
        let title = row[col_idx(&cols, "title")].clone();
        map_value(&mut row[col_idx(&cols, "milestone_id")], milestones_map);
        let mid = &row[col_idx(&cols, "milestone_id")];
        let exists = conn
            .query_row(
                "SELECT 1 FROM plans WHERE title = ?1 AND milestone_id IS ?2",
                params_from_iter([&title, mid]),
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        let target_id = if exists {
            report.skipped += 1;
            conn.query_row(
                "SELECT id FROM plans WHERE title = ?1 AND milestone_id IS ?2",
                params_from_iter([&title, mid]),
                |r| r.get(0),
            )?
        } else {
            let new_id = if let Some(orig) = orig_id {
                if id_taken(conn, "plans", orig)? {
                    next_id(conn, "plans")?
                } else {
                    orig
                }
            } else {
                next_id(conn, "plans")?
            };
            set_id(&mut row, &cols, new_id);
            insert_row(conn, "plans", &cols, &row)?;
            report.inserted += 1;
            new_id
        };
        if let Some(orig) = orig_id {
            id_map.insert(orig, target_id);
        }
    }
    Ok(())
}

/// issues：按 uid 合并——已存在则 LWW（updated_at 新覆盖）；否则插入（id 冲突重映射，引用映射）。
fn merge_issues(
    conn: &Connection,
    tmp: &Connection,
    projects_map: &HashMap<i64, i64>,
    plans_map: &HashMap<i64, i64>,
    id_map: &mut HashMap<i64, i64>,
    report: &mut MergeReport,
) -> Result<(), Error> {
    let cols = columns(tmp, "issues")?;
    for mut row in read_rows(tmp, "issues", &cols)? {
        let orig_id = row_id(&row, &cols);
        let uid = row[col_idx(&cols, "uid")].clone();
        map_value(&mut row[col_idx(&cols, "project_id")], projects_map);
        map_value(&mut row[col_idx(&cols, "plan_id")], plans_map);
        let target_id = match uid {
            Value::Text(u) => {
                let existing: Option<i64> = conn
                    .query_row("SELECT id FROM issues WHERE uid = ?1", [u], |r| r.get(0))
                    .optional()?;
                match existing {
                    Some(id) => {
                        let cur: String = conn
                            .query_row("SELECT updated_at FROM issues WHERE id = ?1", [id], |r| r.get(0))?;
                        let new_upd = match &row[col_idx(&cols, "updated_at")] {
                            Value::Text(s) => s.clone(),
                            _ => String::new(),
                        };
                        if new_upd > cur {
                            update_row(conn, "issues", &cols, &row, id)?;
                            report.updated += 1;
                        } else {
                            report.skipped += 1;
                        }
                        id
                    }
                    None => {
                        let new_id = if let Some(orig) = orig_id {
                            if id_taken(conn, "issues", orig)? {
                                next_id(conn, "issues")?
                            } else {
                                orig
                            }
                        } else {
                            next_id(conn, "issues")?
                        };
                        set_id(&mut row, &cols, new_id);
                        insert_row(conn, "issues", &cols, &row)?;
                        report.inserted += 1;
                        new_id
                    }
                }
            }
            // uid 为空（不应出现）：按 id 幂等兜底，避免重复。
            Value::Integer(id) => {
                report.skipped += 1;
                id
            }
            _ => {
                report.skipped += 1;
                0
            }
        };
        if let Some(orig) = orig_id {
            id_map.insert(orig, target_id);
        }
    }
    Ok(())
}

/// 关联表（issue_labels/issue_links/milestone_direct_issues）：引用 id 映射后 INSERT OR IGNORE。
fn merge_assoc(
    conn: &Connection,
    tmp: &Connection,
    table: &str,
    id_cols: &[&str],
    map_a: &HashMap<i64, i64>,
    map_b: &HashMap<i64, i64>,
    report: &mut MergeReport,
) -> Result<(), Error> {
    let cols = columns(tmp, table)?;
    let mut stmt = conn.prepare(&format!(
        "INSERT OR IGNORE INTO {table} ({}) VALUES ({})",
        cols.join(", "),
        (1..=cols.len()).map(|i| format!("?{i}")).collect::<Vec<_>>().join(", ")
    ))?;
    for mut row in read_rows(tmp, table, &cols)? {
        map_value(&mut row[col_idx(&cols, id_cols[0])], map_a);
        map_value(&mut row[col_idx(&cols, id_cols[1])], map_b);
        match stmt.execute(params_from_iter(row.iter())) {
            Ok(1) => report.inserted += 1,
            _ => report.skipped += 1,
        }
    }
    Ok(())
}

// ── 工具 ────────────────────────────────────────────────────

/// 临时库路径：系统 temp + pid + 时间纳秒（零依赖，用完删除）。
fn temp_db_path() -> Result<std::path::PathBuf, Error> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| Error::Other(format!("clock error: {e}")))?
        .as_nanos();
    Ok(std::env::temp_dir().join(format!(
        "mint-sync-{}-{nanos}.db",
        std::process::id()
    )))
}

/// 取表列名（PRAGMA table_info，定义序）。
fn columns(conn: &Connection, table: &str) -> Result<Vec<String>, Error> {
    let sql = format!("PRAGMA table_info({table})");
    let mut stmt = conn.prepare(&sql)?;
    let cols = stmt
        .query_map([], |r| r.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(cols)
}

/// 读全表行（rowid 序，确定性）。
fn read_rows(conn: &Connection, table: &str, cols: &[String]) -> Result<Vec<Vec<Value>>, Error> {
    let sql = format!("SELECT * FROM {table}");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([], |r| {
            let mut row = Vec::with_capacity(cols.len());
            for i in 0..cols.len() {
                row.push(r.get::<_, Value>(i)?);
            }
            Ok(row)
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// 列索引。
fn col_idx(cols: &[String], name: &str) -> usize {
    cols.iter().position(|c| c == name).unwrap_or(0)
}

/// 目标库指定 id 是否已被占用。
fn id_taken(conn: &Connection, table: &str, id: i64) -> Result<bool, Error> {
    let sql = format!("SELECT 1 FROM {table} WHERE id = ?1");
    Ok(conn
        .query_row(&sql, [id], |_| Ok(()))
        .optional()?
        .is_some())
}

/// 目标库下一个自增 id。
fn next_id(conn: &Connection, table: &str) -> Result<i64, Error> {
    let sql = format!("SELECT COALESCE(MAX(id), 0) + 1 FROM {table}");
    Ok(conn.query_row(&sql, [], |r| r.get(0))?)
}

/// 替换行中 id 列值。
fn set_id(row: &mut [Value], cols: &[String], new_id: i64) {
    row[col_idx(cols, "id")] = Value::Integer(new_id);
}

/// 行中原始 id（快照内 id，未重映射前）。
fn row_id(row: &[Value], cols: &[String]) -> Option<i64> {
    match &row[col_idx(cols, "id")] {
        Value::Integer(i) => Some(*i),
        _ => None,
    }
}

/// 把 Value::Integer 按映射转换（引用列重映射）。
fn map_value(v: &mut Value, map: &HashMap<i64, i64>) {
    if let Value::Integer(i) = v {
        if let Some(&new) = map.get(&i) {
            *v = Value::Integer(new);
        }
    }
}

/// 参数化 INSERT（cols 列，row 值）。
fn insert_row(conn: &Connection, table: &str, cols: &[String], row: &[Value]) -> Result<(), Error> {
    let placeholders = (1..=cols.len()).map(|i| format!("?{i}")).collect::<Vec<_>>().join(", ");
    let sql = format!("INSERT INTO {table} ({}) VALUES ({placeholders})", cols.join(", "));
    conn.execute(&sql, params_from_iter(row.iter()))?;
    Ok(())
}

/// 参数化 UPDATE（除 id 外全部列，WHERE id = ?N）。
fn update_row(
    conn: &Connection,
    table: &str,
    cols: &[String],
    row: &[Value],
    id: i64,
) -> Result<(), Error> {
    let mut sets = Vec::new();
    let mut params: Vec<Value> = Vec::new();
    for (i, c) in cols.iter().enumerate() {
        if c == "id" {
            continue;
        }
        sets.push(format!("{c} = ?{}", params.len() + 1));
        params.push(row[i].clone());
    }
    params.push(Value::Integer(id));
    let sql = format!("UPDATE {table} SET {} WHERE id = ?{}", sets.join(", "), params.len());
    conn.execute(&sql, params_from_iter(params.iter()))?;
    Ok(())
}
