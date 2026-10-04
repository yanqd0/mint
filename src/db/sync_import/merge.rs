//! 业务键合并：machines/keyed/labels/plans 幂等 upsert 与关联表合并。

use std::collections::HashMap;

use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, params_from_iter};

use crate::error::Error;

use super::MergeReport;
use super::merge_issues::merge_issues;
use super::rows::{
    col_idx, columns, fill_null_from_local, id_taken, insert_row, map_value, next_id, read_rows,
    row_id, set_id, update_row,
};

pub(super) fn merge_all(conn: &Connection, tmp: &Connection) -> Result<MergeReport, Error> {
    conn.execute_batch("BEGIN IMMEDIATE")?;
    let res = merge_all_inner(conn, tmp);
    match &res {
        Ok(_) => conn.execute_batch("COMMIT")?,
        // 显式 ROLLBACK（不依赖 Connection drop 的隐式回滚，#401）。
        Err(_) => {
            conn.execute_batch("ROLLBACK").ok();
        }
    }
    res
}

pub(super) fn merge_all_inner(conn: &Connection, tmp: &Connection) -> Result<MergeReport, Error> {
    let mut report = MergeReport::default();
    let mut projects_map = HashMap::new();
    let mut labels_map = HashMap::new();
    let mut milestones_map = HashMap::new();
    let mut plans_map = HashMap::new();
    let mut issues_map = HashMap::new();

    merge_machines(conn, tmp, &mut report)?;
    merge_keyed(
        conn,
        tmp,
        "projects",
        "name",
        &mut projects_map,
        &mut report,
    )?;
    merge_keyed(conn, tmp, "labels", "name", &mut labels_map, &mut report)?;
    merge_keyed(
        conn,
        tmp,
        "milestones",
        "version",
        &mut milestones_map,
        &mut report,
    )?;
    merge_plans(conn, tmp, &milestones_map, &mut plans_map, &mut report)?;
    merge_issues(conn, tmp, &plans_map, &mut issues_map, &mut report)?;
    merge_assoc(
        conn,
        tmp,
        "issue_labels",
        &["issue_id", "label_id"],
        &issues_map,
        &labels_map,
        &mut report,
    )?;
    merge_assoc(
        conn,
        tmp,
        "issue_links",
        &["from_id", "to_id"],
        &issues_map,
        &issues_map,
        &mut report,
    )?;
    merge_assoc(
        conn,
        tmp,
        "milestone_direct_issues",
        &["milestone_id", "issue_id"],
        &milestones_map,
        &issues_map,
        &mut report,
    )?;

    Ok(report)
}

// ── 表专用合并 ──────────────────────────────────────────────

/// machines：PK=machine_id（文本），存在即跳过。
pub(super) fn merge_machines(
    conn: &Connection,
    tmp: &Connection,
    report: &mut MergeReport,
) -> Result<(), Error> {
    let cols = columns(tmp, "machines")?;
    for row in read_rows(tmp, "machines", &cols)? {
        let key = &row[col_idx(&cols, "machine_id")];
        let exists = conn
            .query_row(
                "SELECT 1 FROM machines WHERE machine_id = ?1",
                [key],
                |_| Ok(()),
            )
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
pub(super) fn merge_keyed(
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

/// plans：稳定键 uid + updated_at LWW（#498）；旧快照无 uid 时回退业务键 (title, milestone_id)。
/// milestone_id 经 milestones 映射；无 UNIQUE，用 EXISTS 判定。原实现命中即 skip，导致
/// A 机显式 `plan drop` 的状态无法传播到 C 机（0 updated）；改为命中后按 updated_at 取新。
pub(super) fn merge_plans(
    conn: &Connection,
    tmp: &Connection,
    milestones_map: &HashMap<i64, i64>,
    id_map: &mut HashMap<i64, i64>,
    report: &mut MergeReport,
) -> Result<(), Error> {
    let cols = columns(tmp, "plans")?;
    let uid_idx = cols.iter().position(|c| c == "uid");
    for mut row in read_rows(tmp, "plans", &cols)? {
        let orig_id = row_id(&row, &cols);
        let title = row[col_idx(&cols, "title")].clone();
        map_value(&mut row[col_idx(&cols, "milestone_id")], milestones_map);
        let mid = &row[col_idx(&cols, "milestone_id")];
        // 1) uid 命中（稳定键）；2) 回退业务键 (title, milestone_id)（旧快照/未回填 uid）。
        let by_uid: Option<i64> = match uid_idx.map(|i| &row[i]) {
            Some(Value::Text(u)) => conn
                .query_row("SELECT id FROM plans WHERE uid = ?1", [u], |r| r.get(0))
                .optional()?,
            _ => None,
        };
        let existing = match by_uid {
            Some(id) => Some(id),
            None => conn
                .query_row(
                    "SELECT id FROM plans WHERE title = ?1 AND milestone_id IS ?2",
                    params_from_iter([&title, mid]),
                    |r| r.get(0),
                )
                .optional()?,
        };
        let target_id = match existing {
            Some(id) => {
                let cur: String =
                    conn.query_row("SELECT updated_at FROM plans WHERE id = ?1", [id], |r| {
                        r.get(0)
                    })?;
                let new_upd = match &row[col_idx(&cols, "updated_at")] {
                    Value::Text(s) => s.clone(),
                    _ => String::new(),
                };
                if new_upd > cur {
                    // 快照缺列（旧版导出）→ NULL：保留本地 uid / 手动 drop 标记 / 显式 rank，不抹掉。
                    fill_null_from_local(
                        conn,
                        "plans",
                        &cols,
                        &mut row,
                        id,
                        &["uid", "manual_dropped", "sort_order"],
                    )?;
                    update_row(conn, "plans", &cols, &row, id)?;
                    report.updated += 1;
                } else {
                    report.skipped += 1;
                }
                id
            }
            None => {
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
            }
        };
        if let Some(orig) = orig_id {
            id_map.insert(orig, target_id);
        }
    }
    Ok(())
}

/// 关联表（issue_labels/issue_links/milestone_direct_issues）：引用 id 映射后 INSERT OR IGNORE。
pub(super) fn merge_assoc(
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
        (1..=cols.len())
            .map(|i| format!("?{i}"))
            .collect::<Vec<_>>()
            .join(", ")
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
