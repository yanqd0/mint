//! 快照导出的 SQL 生成细节：schema 导出、幂等改写、数据行导出与列工具。

use rusqlite::Connection;
use rusqlite::types::Value;

use crate::error::Error;

use super::DATA_TABLES;

/// schema 创建顺序：表（依赖序）→ 索引 → 触发器，全部改写 IF NOT EXISTS。
/// 依赖序保证外键引用（issues → machines 等）与索引/触发器（依赖表）的重放顺序正确。
const SCHEMA_TABLES: &[&str] = &[
    "projects",
    "labels",
    "milestones",
    "plans",
    "machines",
    "issues",
    "issue_labels",
    "issue_links",
    "milestone_direct_issues",
    "container_links",
    "issues_fts",
];

/// schema 段：表（依赖序）→ 索引 → 触发器。
pub(super) fn export_schema(conn: &Connection, out: &mut String) -> Result<(), Error> {
    let mut stmt = conn.prepare(
        "SELECT type, name, sql FROM sqlite_master \
         WHERE sql IS NOT NULL AND type IN ('table', 'index', 'trigger')",
    )?;
    let rows: Vec<(String, String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<_, _>>()?;
    let sql_of = |ty: &str, name: &str| -> Option<&str> {
        rows.iter()
            .find(|(t, n, _)| t == ty && n == name)
            .map(|(_, _, s)| s.as_str())
    };
    // 表：按依赖序。
    for name in SCHEMA_TABLES {
        if let Some(sql) = sql_of("table", name) {
            out.push_str(&make_idempotent(sql));
            out.push_str(";\n");
        }
    }
    // 索引（依赖表，无相互依赖）。
    for (_ty, _name, sql) in &rows {
        if _ty == "index" {
            out.push_str(&make_idempotent(sql));
            out.push_str(";\n");
        }
    }
    // 触发器（依赖表与 FTS 虚表，已建）。
    for (_ty, _name, sql) in &rows {
        if _ty == "trigger" {
            out.push_str(&make_idempotent(sql));
            out.push_str(";\n");
        }
    }
    Ok(())
}

/// 把 CREATE 语句改写为 IF NOT EXISTS（表/虚表/索引/触发器）。
pub(super) fn make_idempotent(sql: &str) -> String {
    if sql.starts_with("CREATE VIRTUAL TABLE") {
        sql.replacen(
            "CREATE VIRTUAL TABLE",
            "CREATE VIRTUAL TABLE IF NOT EXISTS",
            1,
        )
    } else if sql.starts_with("CREATE TABLE") {
        sql.replacen("CREATE TABLE", "CREATE TABLE IF NOT EXISTS", 1)
    } else if sql.starts_with("CREATE UNIQUE INDEX") {
        sql.replacen(
            "CREATE UNIQUE INDEX",
            "CREATE UNIQUE INDEX IF NOT EXISTS",
            1,
        )
    } else if sql.starts_with("CREATE INDEX") {
        sql.replacen("CREATE INDEX", "CREATE INDEX IF NOT EXISTS", 1)
    } else if sql.starts_with("CREATE TRIGGER") {
        sql.replacen("CREATE TRIGGER", "CREATE TRIGGER IF NOT EXISTS", 1)
    } else {
        sql.to_string()
    }
}

/// 列信息（PRAGMA table_info 的 name + 主键序号）。
pub(super) struct Column {
    name: String,
    pk: i64,
}

/// data 段：逐表按主键排序导出 INSERT（每行一条，git diff 友好）。
pub(super) fn export_data(conn: &Connection, out: &mut String) -> Result<(), Error> {
    for table in DATA_TABLES {
        export_table(conn, out, table, None, &[])?;
    }
    Ok(())
}

/// 导出单表（可选 WHERE 过滤 + 排除列；列序来自 PRAGMA table_info，行序按主键升序）。
/// `exclude` 用于旧库（003 含 project_id）导出到新 schema（004 无 project_id）时跳过列。
pub(super) fn export_table(
    conn: &Connection,
    out: &mut String,
    table: &str,
    filter: Option<&str>,
    exclude: &[&str],
) -> Result<(), Error> {
    let mut cols = table_columns(conn, table)?;
    cols.retain(|c| !exclude.contains(&c.name.as_str()));
    let order = pk_clause(&cols);
    let where_sql = filter.map(|f| format!(" WHERE {f}")).unwrap_or_default();
    // 显式列（非 SELECT *）：exclude 后行列序与 cols 一致，避免错位。
    let col_list = cols
        .iter()
        .map(|c| c.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let sel = format!("SELECT {col_list} FROM {table}{where_sql} ORDER BY {order}");
    let mut stmt = conn.prepare(&sel)?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        out.push_str(&format!("INSERT INTO {table} ("));
        out.push_str(
            &cols
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        );
        out.push_str(") VALUES (");
        for (i, _c) in cols.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            let v: Value = row.get(i)?;
            out.push_str(&sql_value(&v));
        }
        out.push_str(");\n");
    }
    Ok(())
}

/// 表是否存在于该连接（`export_sql_for_project` 处理旧版 legacy db 时用：旧库可能没有后加的表）。
pub(super) fn table_exists(conn: &Connection, table: &str) -> Result<bool, Error> {
    let n: i64 = conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [table],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

/// 取表列（按定义顺序；name + pk 序号）。
pub(super) fn table_columns(conn: &Connection, table: &str) -> Result<Vec<Column>, Error> {
    let sql = format!("PRAGMA table_info({table})");
    let mut stmt = conn.prepare(&sql)?;
    let cols: Vec<Column> = stmt
        .query_map([], |r| {
            Ok(Column {
                name: r.get(1)?,
                pk: r.get(5)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(cols)
}

/// 主键排序子句（pk 序号升序；无主键回退 rowid）。
pub(super) fn pk_clause(cols: &[Column]) -> String {
    let mut pks: Vec<(&str, i64)> = cols
        .iter()
        .filter(|c| c.pk > 0)
        .map(|c| (c.name.as_str(), c.pk))
        .collect();
    pks.sort_by_key(|(_, p)| *p);
    if pks.is_empty() {
        "rowid".to_string()
    } else {
        pks.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ")
    }
}

/// 值转义：NULL/数字原样；字符串单引号加倍；BLOB 十六进制。
pub(super) fn sql_value(v: &Value) -> String {
    match v {
        Value::Null => "NULL".to_string(),
        Value::Integer(i) => i.to_string(),
        Value::Real(f) => f.to_string(),
        Value::Text(s) => format!("'{}'", s.replace('\'', "''")),
        Value::Blob(b) => format!(
            "X'{}'",
            b.iter().map(|x| format!("{x:02x}")).collect::<String>()
        ),
    }
}
