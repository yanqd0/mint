//! 打开与 WAL：权限、幂等 checkpoint、外键与索引使用。

use super::*;

/// wal_checkpoint：真实文件库写后 TRUNCATE 使 WAL 归零（#299，尽力而为）。
#[test]
fn wal_checkpoint_truncate_empties_wal() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cp.db");
    let conn = crate::db::open(&path).unwrap();
    // 显式写产生 WAL 内容（默认 wal_autocheckpoint=1000 页，小写不触发）。
    conn.execute_batch("CREATE TABLE t(x); INSERT INTO t VALUES (1);")
        .unwrap();
    let wal = path.with_extension("db-wal");
    crate::db::wal_checkpoint(&conn, false); // PASSIVE 不 panic
    crate::db::wal_checkpoint(&conn, true); // TRUNCATE 后 WAL 归零
    let size = std::fs::metadata(&wal).map(|m| m.len()).unwrap_or(0);
    assert_eq!(size, 0, "TRUNCATE 后 WAL 应归零: {size}");
}

/// EXPLAIN QUERY PLAN：issues.plan_id 过滤命中 idx_issues_plan_id（#423，#300 索引被使用）。
#[test]
fn explain_plan_id_filter_uses_index() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    conn.execute_batch(
        "INSERT INTO projects (name) VALUES ('p'); INSERT INTO issues (title) VALUES ('x');",
    )
    .unwrap();
    let detail: Vec<String> = conn
        .prepare("EXPLAIN QUERY PLAN SELECT * FROM issues WHERE plan_id = 1")
        .unwrap()
        .query_map([], |r| r.get(3))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(
        detail.iter().any(|d| d.contains("idx_issues_plan_id")),
        "plan_id 过滤应命中索引: {detail:?}"
    );
}

/// 目录 0700 + 文件 0600：DB 权限收敛（敏感开发数据仅本用户可读）。
#[test]
fn open_restricts_db_permissions() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("nested/m.db");
    open(&db).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let dir_mode = std::fs::metadata(dir.path().join("nested"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(dir_mode, 0o700, "目录应为 0700");
        let file_mode = std::fs::metadata(&db).unwrap().permissions().mode() & 0o777;
        assert_eq!(file_mode, 0o600, "文件应为 0600");
    }
}

/// 外键约束生效（默认关闭，需 PRAGMA foreign_keys）。
#[test]
fn foreign_keys_enforced_when_enabled() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    migrate(&conn).unwrap();

    let err = conn
        .execute("INSERT INTO issues (title, plan_id) VALUES ('x', 999)", [])
        .unwrap_err();
    assert!(err.to_string().contains("FOREIGN KEY"));
}

/// issues.kind 无 DB CHECK：task 可插入并回读（本 commit 目标）；非法值 DB 放行但 FromSql 报 invalid kind（应用层兜底）。
#[test]
fn kind_has_no_db_check_and_fromsql_guards() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();

    let ddl: String = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='issues'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!ddl.contains("kind IN"), "kind 不应有 DB CHECK：{ddl}");

    conn.execute("INSERT INTO projects (name) VALUES ('p')", [])
        .unwrap();

    // task 可插入并回读（去 CHECK 的目标）
    conn.execute("INSERT INTO issues (title, kind) VALUES ('t', 'task')", [])
        .unwrap();
    let got: crate::models::Kind = conn
        .query_row("SELECT kind FROM issues WHERE id = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(got, crate::models::Kind::Task);

    // 非法值 DB 放行，但 FromSql 读取报 invalid kind（应用层兜底）
    conn.execute("INSERT INTO issues (title, kind) VALUES ('b', 'bogus')", [])
        .unwrap();
    let err = conn
        .query_row::<crate::models::Kind, _, _>("SELECT kind FROM issues WHERE id = 2", [], |r| {
            r.get(0)
        })
        .unwrap_err();
    assert!(err.to_string().contains("invalid kind"), "{err}");
}
