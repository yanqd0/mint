//! 关联/链接表合并：issue_labels / issue_links / milestone_direct_issues（通用）
//! 与 container_links（容器级 blocks，kind 感知映射）。
//!
//! 自 `merge.rs` 拆出（>300 行门禁）：只移动代码，语义与调用顺序不变。

use std::collections::HashMap;

use rusqlite::types::Value;
use rusqlite::{Connection, params_from_iter};

use crate::error::Error;

use super::super::MergeReport;
use super::super::rows::{col_idx, columns, map_value, read_rows};

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

/// 容器级链接（container_links）：按行内 kind 选映射表（plan/milestone 各自 id 空间）后
/// INSERT OR IGNORE。导出端已保证两端都在本次快照的容器集合内，故映射必然命中。
pub(super) fn merge_container_links(
    conn: &Connection,
    tmp: &Connection,
    milestones_map: &HashMap<i64, i64>,
    plans_map: &HashMap<i64, i64>,
    report: &mut MergeReport,
) -> Result<(), Error> {
    let cols = columns(tmp, "container_links")?;
    let mut stmt = conn.prepare(&format!(
        "INSERT OR IGNORE INTO container_links ({}) VALUES ({})",
        cols.join(", "),
        (1..=cols.len())
            .map(|i| format!("?{i}"))
            .collect::<Vec<_>>()
            .join(", ")
    ))?;
    for mut row in read_rows(tmp, "container_links", &cols)? {
        let map = match &row[col_idx(&cols, "kind")] {
            Value::Text(k) if k == "plan" => plans_map,
            Value::Text(k) if k == "milestone" => milestones_map,
            // 未知 kind（DDL CHECK 下不应出现）：跳过而非整体失败。
            _ => {
                report.skipped += 1;
                continue;
            }
        };
        map_value(&mut row[col_idx(&cols, "from_id")], map);
        map_value(&mut row[col_idx(&cols, "to_id")], map);
        match stmt.execute(params_from_iter(row.iter())) {
            Ok(1) => report.inserted += 1,
            _ => report.skipped += 1,
        }
    }
    Ok(())
}
