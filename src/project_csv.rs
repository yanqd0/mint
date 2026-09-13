//! project 的 CSV 字段拼装/解析（paths/repo_url 多值字段）。

use rusqlite::{Connection, params};

use crate::error::Error;

/// 可追加的 project CSV 字段（git / abs_dir）——白名单枚举化，消除任意字符串拼 SQL 列名。
pub(super) enum CsvField {
    Git,
    AbsDir,
}

impl CsvField {
    /// 固定白名单列名（非用户输入），供 SELECT/UPDATE 拼接。
    fn col(&self) -> &'static str {
        match self {
            CsvField::Git => "git",
            CsvField::AbsDir => "abs_dir",
        }
    }
}

/// CSV 单元格编码：含逗号/引号/换行时引号包裹 + 内部引号加倍（RFC 4180 子集），
/// 避免含逗号路径（如 abs_dir `/path/with,comma`）被 split 误拆致每次 ensure 重复追加。
pub(super) fn csv_escape(v: &str) -> String {
    if v.contains(',') || v.contains('"') || v.contains('\n') {
        format!("\"{}\"", v.replace('"', "\"\""))
    } else {
        v.to_string()
    }
}

/// 解析 CSV 行（引号包裹 + 双引号转义），返回字段列表。
pub(super) fn csv_parse(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => fields.push(std::mem::take(&mut cur)),
            c => cur.push(c),
        }
    }
    fields.push(cur);
    fields
}

/// 追加值到 CSV 字段（不存在时追加，逗号分隔；含逗号/引号值转义存储）。
pub(super) fn append_csv(
    conn: &Connection,
    id: i64,
    field: CsvField,
    value: &str,
) -> Result<(), Error> {
    let col = field.col(); // 白名单列名："git" / "abs_dir"
    let current: String = conn
        .query_row(
            &format!("SELECT {col} FROM projects WHERE id = ?1"),
            params![id],
            |r| r.get(0),
        )
        .unwrap_or_default();
    let current = current.trim();
    if current.is_empty() {
        conn.execute(
            &format!("UPDATE projects SET {col} = ?2, updated_at = datetime('now') WHERE id = ?1"),
            params![id, csv_escape(value)],
        )?;
    } else if !csv_parse(current).iter().any(|s| s == value) {
        let merged = format!("{current},{}", csv_escape(value));
        conn.execute(
            &format!("UPDATE projects SET {col} = ?2, updated_at = datetime('now') WHERE id = ?1"),
            params![id, merged],
        )?;
    }
    Ok(())
}
