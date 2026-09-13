//! sync.rs 拆分的独立测试模块。

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
        "INSERT INTO issues (id, title) VALUES (?1, ?2)",
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
        sql.contains("CREATE TABLE IF NOT EXISTS"),
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
