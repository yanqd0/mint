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
struct Column {
    name: String,
    pk: i64,
}

/// data 段：逐表按主键排序导出 INSERT（每行一条，git diff 友好）。
fn export_data(conn: &Connection, out: &mut String) -> Result<(), Error> {
    for table in DATA_TABLES {
        export_table(conn, out, table, None)?;
    }
    Ok(())
}

/// 导出单表（可选 WHERE 过滤；列序来自 PRAGMA table_info，行序按主键升序）。
fn export_table(
    conn: &Connection,
    out: &mut String,
    table: &str,
    filter: Option<&str>,
) -> Result<(), Error> {
    let cols = table_columns(conn, table)?;
    let order = pk_clause(&cols);
    let where_sql = filter.map(|f| format!(" WHERE {f}")).unwrap_or_default();
    let sel = format!("SELECT * FROM {table}{where_sql} ORDER BY {order}");
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

/// 按 project 导出自包含快照（migrate_split 用）：schema 全量 + 该项目数据过滤。
/// 依赖序满足外键（machines→projects→labels→milestones→plans→issues→关联表）；
/// machines 全量导出（机器级元数据每项目 db 保留一份；也是临时库 FK 检查的必要前置）。
pub fn export_sql_for_project(conn: &Connection, project_id: i64) -> Result<String, Error> {
    let mut out = String::new();
    out.push_str(&format!(
        "-- mint sync snapshot v1 ({}) for project {project_id}\n",
        crate::db::machine_id()
    ));
    out.push_str("-- schema\n");
    export_schema(conn, &mut out)?;
    out.push_str("-- data\n");
    let pid = project_id.to_string();
    // machines 全量（旧 db 的本机/历史机器）；labels 按引用该项目的 issues 复制（保留原 id）。
    export_table(conn, &mut out, "machines", None)?;
    export_table(conn, &mut out, "projects", Some(&format!("id = {pid}")))?;
    export_table(
        conn,
        &mut out,
        "labels",
        Some(&format!(
            "id IN (SELECT il.label_id FROM issue_labels il JOIN issues i ON i.id = il.issue_id WHERE i.project_id = {pid})"
        )),
    )?;
    export_table(
        conn,
        &mut out,
        "milestones",
        Some(&format!(
            "id IN (SELECT milestone_id FROM plans WHERE id IN (SELECT plan_id FROM issues WHERE project_id = {pid} AND plan_id IS NOT NULL)) \
             OR id IN (SELECT milestone_id FROM milestone_direct_issues WHERE issue_id IN (SELECT id FROM issues WHERE project_id = {pid}))"
        )),
    )?;
    export_table(
        conn,
        &mut out,
        "plans",
        Some(&format!(
            "id IN (SELECT plan_id FROM issues WHERE project_id = {pid} AND plan_id IS NOT NULL)"
        )),
    )?;
    export_table(
        conn,
        &mut out,
        "issues",
        Some(&format!("project_id = {pid}")),
    )?;
    export_table(
        conn,
        &mut out,
        "issue_labels",
        Some(&format!(
            "issue_id IN (SELECT id FROM issues WHERE project_id = {pid})"
        )),
    )?;
    export_table(
        conn,
        &mut out,
        "issue_links",
        Some(&format!(
            "from_id IN (SELECT id FROM issues WHERE project_id = {pid}) AND to_id IN (SELECT id FROM issues WHERE project_id = {pid})"
        )),
    )?;
    export_table(
        conn,
        &mut out,
        "milestone_direct_issues",
        Some(&format!(
            "issue_id IN (SELECT id FROM issues WHERE project_id = {pid})"
        )),
    )?;
    Ok(out)
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
        Value::Blob(b) => format!(
            "X'{}'",
            b.iter().map(|x| format!("{x:02x}")).collect::<String>()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{Connection, params};

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::migrate_for_test(&conn);
        conn.execute("INSERT INTO projects (name) VALUES ('p')", [])
            .unwrap();
        conn
    }

    fn add_issue(conn: &Connection, id: i64, title: &str) {
        conn.execute(
            "INSERT INTO issues (id, title, project_id) VALUES (?1, ?2, 1)",
            params![id, title],
        )
        .unwrap();
    }

    /// 确定性：同一库两次导出字节一致（主键排序 + 固定列序 + 一致转义）。
    #[test]
    fn export_deterministic() {
        let conn = test_conn();
        add_issue(&conn, 2, "b");
        add_issue(&conn, 1, "a");
        let s1 = export_sql(&conn).unwrap();
        let s2 = export_sql(&conn).unwrap();
        assert_eq!(s1, s2);
    }

    /// schema 段含表/索引/触发器（IF NOT EXISTS）；数据段含 INSERT 且主键升序。
    #[test]
    fn export_has_schema_and_sorted_data() {
        let conn = test_conn();
        add_issue(&conn, 2, "b");
        add_issue(&conn, 1, "a");
        let sql = export_sql(&conn).unwrap();
        assert!(
            sql.contains("CREATE TABLE IF NOT EXISTS issues"),
            "缺 schema: {sql}"
        );
        assert!(sql.contains("INSERT INTO issues"), "缺数据段");
        let a_pos = sql.find("'a'").unwrap();
        let b_pos = sql.find("'b'").unwrap();
        assert!(a_pos < b_pos, "主键升序：a(1) 应在 b(2) 前");
    }

    /// 单引号转义为双引号（''），不破坏 SQL 语法。
    #[test]
    fn export_escapes_quotes() {
        let conn = test_conn();
        add_issue(&conn, 1, "it's");
        let sql = export_sql(&conn).unwrap();
        assert!(sql.contains("'it''s'"), "单引号应翻倍: {sql}");
    }

    /// 数据段跳过 FTS 虚表（触发器维护）；schema 段含 FTS 虚表定义（空库重建需）。
    #[test]
    fn export_skips_fts_data_but_keeps_schema() {
        let conn = test_conn();
        add_issue(&conn, 1, "a");
        let sql = export_sql(&conn).unwrap();
        assert!(
            sql.contains("CREATE VIRTUAL TABLE IF NOT EXISTS issues_fts"),
            "缺 FTS schema"
        );
        // 数据段（-- data 之后）不得含 FTS INSERT（schema 段触发器体含其文本，故只看数据段）。
        let data = sql.split("-- data").nth(1).unwrap_or("");
        assert!(
            !data.contains("INSERT INTO issues_fts"),
            "数据段不应含 FTS INSERT"
        );
    }
}
