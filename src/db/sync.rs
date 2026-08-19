//! 同步快照导出 / 导入（git+SQL 路线，plan #84）。
//!
//! 确定性导出：schema（全部 IF NOT EXISTS）+ 数据（按主键排序），跳过 FTS 数据（触发器维护）。
//! 导入：幂等合并到本机库（uid/LWW + id 重映射），见 `import_sql`。

use rusqlite::{Connection, types::Value};

use crate::error::Error;

/// 参与导出的数据表（FTS 虚表由触发器维护，跳过）。
const DATA_TABLES: &[&str] = &[
    "machines",
    "projects",
    "labels",
    "milestones",
    "plans",
    "issues",
    "issue_labels",
    "issue_links",
    "milestone_direct_issues",
];

/// 导出确定性 SQL 快照文本（标准 SQL，可直接被 sqlite3 执行）。
pub fn export_sql(conn: &Connection) -> Result<String, Error> {
    let mut out = String::new();
    out.push_str(&format!(
        "-- mint sync snapshot v1 ({})\n",
        crate::db::machine_id()
    ));
    out.push_str("-- schema\n");
    export_schema(conn, &mut out)?;
    out.push_str("-- data\n");
    export_data(conn, &mut out)?;
    Ok(out)
}

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
    "issues_fts",
];

/// schema 段：表（依赖序）→ 索引 → 触发器。
fn export_schema(conn: &Connection, out: &mut String) -> Result<(), Error> {
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
fn make_idempotent(sql: &str) -> String {
    if sql.starts_with("CREATE VIRTUAL TABLE") {
        sql.replacen("CREATE VIRTUAL TABLE", "CREATE VIRTUAL TABLE IF NOT EXISTS", 1)
    } else if sql.starts_with("CREATE TABLE") {
        sql.replacen("CREATE TABLE", "CREATE TABLE IF NOT EXISTS", 1)
    } else if sql.starts_with("CREATE UNIQUE INDEX") {
        sql.replacen("CREATE UNIQUE INDEX", "CREATE UNIQUE INDEX IF NOT EXISTS", 1)
    } else if sql.starts_with("CREATE INDEX") {
        sql.replacen("CREATE INDEX", "CREATE INDEX IF NOT EXISTS", 1)
    } else if sql.starts_with("CREATE TRIGGER") {
        sql.replacen("CREATE TRIGGER", "CREATE TRIGGER IF NOT EXISTS", 1)
    } else {
        sql.to_string()
    }
}

/// 列信息（PRAGMA table_info 的 name + 主键序号）。
struct Column {
    name: String,
    pk: i64,
}

/// data 段：逐表按主键排序导出 INSERT（每行一条，git diff 友好）。
fn export_data(conn: &Connection, out: &mut String) -> Result<(), Error> {
    for table in DATA_TABLES {
        let cols = table_columns(conn, table)?;
        let order = pk_clause(&cols);
        let sel = format!("SELECT * FROM {table} ORDER BY {order}");
        let mut stmt = conn.prepare(&sel)?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            out.push_str(&format!("INSERT INTO {table} ("));
            out.push_str(&cols.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(", "));
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
    }
    Ok(())
}

/// 取表列（按定义顺序；name + pk 序号）。
fn table_columns(conn: &Connection, table: &str) -> Result<Vec<Column>, Error> {
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
fn pk_clause(cols: &[Column]) -> String {
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
fn sql_value(v: &Value) -> String {
    match v {
        Value::Null => "NULL".to_string(),
        Value::Integer(i) => i.to_string(),
        Value::Real(f) => f.to_string(),
        Value::Text(s) => format!("'{}'", s.replace('\'', "''")),
        Value::Blob(b) => format!("X'{}'", b.iter().map(|x| format!("{x:02x}")).collect::<String>()),
    }
}
