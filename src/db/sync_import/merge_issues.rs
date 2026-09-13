//! issues 合并：uid 稳定键 + updated_at LWW + id 冲突重映射。

use std::collections::HashMap;

use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension};

use crate::error::Error;

use super::MergeReport;
use super::rows::{
    col_idx, columns, id_taken, insert_row, map_value, next_id, read_rows, row_id, set_id,
    update_row,
};

/// issues：按 uid 合并——已存在则 LWW（updated_at 新覆盖）；否则插入（id 冲突重映射，引用映射）。
pub(super) fn merge_issues(
    conn: &Connection,
    tmp: &Connection,
    plans_map: &HashMap<i64, i64>,
    id_map: &mut HashMap<i64, i64>,
    report: &mut MergeReport,
) -> Result<(), Error> {
    let cols = columns(tmp, "issues")?;
    for mut row in read_rows(tmp, "issues", &cols)? {
        let orig_id = row_id(&row, &cols);
        let uid = row[col_idx(&cols, "uid")].clone();
        map_value(&mut row[col_idx(&cols, "plan_id")], plans_map);
        let target_id = match uid {
            Value::Text(u) => {
                let existing: Option<i64> = conn
                    .query_row("SELECT id FROM issues WHERE uid = ?1", [u], |r| r.get(0))
                    .optional()?;
                match existing {
                    Some(id) => {
                        let cur: String = conn.query_row(
                            "SELECT updated_at FROM issues WHERE id = ?1",
                            [id],
                            |r| r.get(0),
                        )?;
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
            // uid 为空（002 迁移前历史数据，无 machine_id 无法合成 uid）：按 id 幂等兜底插入，
            // 保留 issue 与其关联（不跳过、不映射 0，否则 FK 违反致迁移失败或关联被吞，#395）。
            Value::Null => {
                let orig = orig_id.ok_or_else(|| {
                    Error::Other("snapshot issue has neither uid nor id".to_string())
                })?;
                if id_taken(conn, "issues", orig)? {
                    report.skipped += 1;
                } else {
                    set_id(&mut row, &cols, orig);
                    insert_row(conn, "issues", &cols, &row)?;
                    report.inserted += 1;
                }
                orig
            }
            // uid 异常非文本（不应出现）：跳过但不映射 0（幂等，避免破坏关联）。
            _ => {
                report.skipped += 1;
                orig_id.unwrap_or(0)
            }
        };
        if let Some(orig) = orig_id {
            id_map.insert(orig, target_id);
        }
    }
    Ok(())
}
