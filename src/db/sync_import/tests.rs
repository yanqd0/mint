//! sync_import 单测：幂等 / LWW / id 重映射 / 恶意快照拒绝。

use super::sanitize::{split_sql_statements, strip_leading_comment};

use super::*;
use rusqlite::{Connection, params};

fn test_conn() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    crate::db::migrate_for_test(&conn);
    conn.execute("INSERT INTO projects (name) VALUES ('p')", [])
        .unwrap();
    conn
}

fn seed_machine(conn: &Connection, mid: &str) {
    conn.execute(
        "INSERT OR IGNORE INTO machines (machine_id, hostname, user) VALUES (?1, 'h', 'u')",
        [mid],
    )
    .unwrap();
}

fn seed_issue(conn: &Connection, id: i64, uid: &str, title: &str, updated: &str) {
    let mid = uid.split(':').next().unwrap();
    seed_machine(conn, mid);
    conn.execute(
        "INSERT INTO issues (id, title, machine_id, uid, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, '2026-01-01 00:00:00', ?5)",
        params![id, title, mid, uid, updated],
    )
    .unwrap();
}

/// 造 plan 行（uid 可空，模拟未回填/旧快照）。
fn seed_plan(
    conn: &Connection,
    id: i64,
    uid: Option<&str>,
    title: &str,
    status: &str,
    updated: &str,
) {
    conn.execute(
        "INSERT INTO plans (id, title, uid, status, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, '2026-01-01 00:00:00', ?5)",
        params![id, title, uid, status, updated],
    )
    .unwrap();
}

/// 空库全量导入：A 快照 → B（空）→ B 含 A 数据。
#[test]
fn import_into_empty() {
    let a = test_conn();
    seed_issue(&a, 1, "mach-a:1", "来自A", "2026-01-01 00:00:00");
    let sql = crate::db::sync::export_sql(&a).unwrap();
    let mut b = test_conn();
    let r = import_sql(&mut b, &sql).unwrap();
    assert!(r.inserted >= 1, "machines+issues 至少插入 1");
    let (cnt, title): (i64, String) = b
        .query_row("SELECT count(*), max(title) FROM issues", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(cnt, 1);
    assert_eq!(title, "来自A");
}

/// 幂等：重复导入不重复插入（uid 键去重）。
#[test]
fn import_idempotent() {
    let a = test_conn();
    seed_issue(&a, 1, "mach-a:1", "来自A", "2026-01-01 00:00:00");
    let sql = crate::db::sync::export_sql(&a).unwrap();
    let mut b = test_conn();
    import_sql(&mut b, &sql).unwrap();
    let r = import_sql(&mut b, &sql).unwrap();
    assert_eq!(r.inserted, 0);
    let cnt: i64 = b
        .query_row("SELECT count(*) FROM issues", [], |r| r.get(0))
        .unwrap();
    assert_eq!(cnt, 1);
}

/// LWW：同 uid 快照 updated_at 更新 → 覆盖目标行（id 保持目标库的）。
#[test]
fn import_lww_updates() {
    let a = test_conn();
    seed_issue(&a, 1, "mach-a:1", "新标题", "2026-02-01 00:00:00");
    let sql = crate::db::sync::export_sql(&a).unwrap();
    let mut b = test_conn();
    seed_issue(&b, 5, "mach-a:1", "旧标题", "2026-01-01 00:00:00");
    let r = import_sql(&mut b, &sql).unwrap();
    assert_eq!(r.updated, 1);
    let (id, title): (i64, String) = b
        .query_row(
            "SELECT id, title FROM issues WHERE uid = 'mach-a:1'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(id, 5, "LWW 保留目标库 id");
    assert_eq!(title, "新标题");
}

/// id 冲突重映射 + 关联（issue_labels）指向重映射后的 id。
#[test]
fn import_id_remap_and_assoc() {
    let a = test_conn();
    seed_issue(&a, 1, "mach-a:1", "A问题", "2026-01-01 00:00:00");
    a.execute("INSERT INTO labels (name) VALUES ('bug')", [])
        .unwrap();
    a.execute(
        "INSERT INTO issue_labels (issue_id, label_id) VALUES (1, 1)",
        [],
    )
    .unwrap();
    let sql = crate::db::sync::export_sql(&a).unwrap();

    let mut b = test_conn();
    seed_issue(&b, 1, "mach-b:1", "B问题", "2026-01-01 00:00:00");
    let r = import_sql(&mut b, &sql).unwrap();
    assert!(r.inserted >= 1);
    // A 的 issue id=1 冲突 → 重映射为 2；关联指向 2。
    let (cnt, remap): (i64, i64) = b
        .query_row(
            "SELECT count(*), coalesce(max(CASE WHEN uid='mach-a:1' THEN id END), 0) FROM issues",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(cnt, 2);
    assert_eq!(remap, 2);
    let (il_issue, il_label): (i64, i64) = b
        .query_row("SELECT issue_id, label_id FROM issue_labels", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(il_issue, 2);
    assert_eq!(il_label, 1);
}

/// 恶意快照被拒：ATTACH/DROP/非白名单表 INSERT 一律拒绝；正常导出快照不受影响（#394）。
#[test]
fn import_rejects_malicious_snapshot() {
    let mut b = test_conn();
    for evil in [
        "INSERT INTO issues (title) VALUES ('x'); ATTACH DATABASE 'evil' AS x;",
        "INSERT INTO issues (title) VALUES ('x'); DROP TABLE issues;",
        "INSERT INTO sqlite_master (type) VALUES ('table');",
        "PRAGMA writable_schema=ON;",
        // 发布审查（security Low）：CREATE TEMP TRIGGER 绕过触发器剔除 → 应拒绝。
        "CREATE TEMP TRIGGER evil AFTER INSERT ON issues BEGIN UPDATE issues SET title='x'; END;",
    ] {
        assert!(import_sql(&mut b, evil).is_err(), "应拒绝恶意快照: {evil}");
    }
    // 正常快照不受影响。
    let a = test_conn();
    a.execute("INSERT INTO issues (title) VALUES ('ok')", [])
        .unwrap();
    let sql = crate::db::sync::export_sql(&a).unwrap();
    assert!(import_sql(&mut b, &sql).is_ok());
    let (cnt, title): (i64, String) = b
        .query_row(
            "SELECT count(*), coalesce(max(title),'') FROM issues",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((cnt, title.as_str()), (1, "ok"));
}

/// 语句分割：单引号字符串与注释内的分号不误拆（#394）。
#[test]
fn split_sql_statements_ignores_inline_semicolons() {
    let sql = "INSERT INTO issues (title) VALUES ('a;b'); -- c; d\nINSERT INTO labels (name) VALUES ('e');";
    let stmts = split_sql_statements(sql);
    assert_eq!(stmts.len(), 2, "{stmts:?}");
    assert!(stmts[0].contains("'a;b'"), "字符串内分号不拆: {stmts:?}");
    // 注释行与后续 INSERT 同一段（注释内分号不切）；strip 后应还原出完整 INSERT。
    let clean = strip_leading_comment(stmts[1]);
    assert!(
        clean.starts_with("INSERT INTO labels"),
        "注释剥离后为 INSERT: {clean:?}"
    );
}

/// NULL-uid 存量 issue（002 前无 machine_id 数据）：按 id 幂等插入，issue 与关联保留（#395）。
#[test]
fn import_null_uid_issue_kept_by_id() {
    let a = test_conn();
    a.execute("INSERT INTO issues (title) VALUES ('legacy')", [])
        .unwrap();
    a.execute("INSERT INTO labels (name) VALUES ('old')", [])
        .unwrap();
    a.execute(
        "INSERT INTO issue_labels (issue_id, label_id) VALUES (1, 1)",
        [],
    )
    .unwrap();
    let sql = crate::db::sync::export_sql(&a).unwrap();
    let mut b = test_conn();
    import_sql(&mut b, &sql).unwrap();
    let (cnt, title): (i64, String) = b
        .query_row(
            "SELECT count(*), coalesce(max(title),'') FROM issues",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(cnt, 1, "NULL-uid issue 不应被丢弃");
    assert_eq!(title, "legacy");
    let n: i64 = b
        .query_row("SELECT count(*) FROM issue_labels", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 1, "NULL-uid issue 的关联应保留");
}

/// issue_links 引用经 id 重映射修正（from/to 都转换）。
#[test]
fn import_links_remap() {
    let a = test_conn();
    seed_issue(&a, 1, "mach-a:1", "A1", "2026-01-01 00:00:00");
    seed_issue(&a, 2, "mach-a:2", "A2", "2026-01-01 00:00:00");
    a.execute(
        "INSERT INTO issue_links (from_id, type, to_id) VALUES (1, 'related', 2)",
        [],
    )
    .unwrap();
    let sql = crate::db::sync::export_sql(&a).unwrap();

    let mut b = test_conn();
    seed_issue(&b, 1, "mach-b:1", "B1", "2026-01-01 00:00:00");
    let r = import_sql(&mut b, &sql).unwrap();
    assert!(r.inserted >= 2);
    // A1(id1→2)、A2(id2→3)；link (1→2) 应映射为 (2→3)。
    let (f, t): (i64, i64) = b
        .query_row("SELECT from_id, to_id FROM issue_links", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!((f, t), (2, 3));
}

/// #498：A 机显式 drop 的 plan 经 uid LWW 传播到 C 机（原实现命中即 skip → 0 updated）。
#[test]
fn import_plan_drop_propagates_by_uid() {
    let a = test_conn();
    seed_plan(
        &a,
        1,
        Some("mach-a:plan:1"),
        "p",
        "dropped",
        "2026-02-01 00:00:00",
    );
    let sql = crate::db::sync::export_sql(&a).unwrap();

    let mut b = test_conn();
    seed_plan(
        &b,
        3,
        Some("mach-a:plan:1"),
        "p",
        "open",
        "2026-01-01 00:00:00",
    );
    let r = import_sql(&mut b, &sql).unwrap();
    assert_eq!(r.updated, 1, "drop 应经 LWW 传播: {r:?}");
    let (id, status, uid): (i64, String, String) = b
        .query_row("SELECT id, status, uid FROM plans", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .unwrap();
    assert_eq!(id, 3, "LWW 保留目标库 id");
    assert_eq!(status, "dropped");
    assert_eq!(uid, "mach-a:plan:1");
}

/// #498：旧快照/未回填 uid 时按 (title, milestone_id) 回退命中，并采纳快照 uid（不新建重复 plan）。
#[test]
fn import_plan_fallback_key_adopts_uid() {
    let a = test_conn();
    seed_plan(
        &a,
        1,
        Some("mach-a:plan:1"),
        "p",
        "dropped",
        "2026-02-01 00:00:00",
    );
    let sql = crate::db::sync::export_sql(&a).unwrap();

    let mut b = test_conn();
    seed_plan(&b, 3, None, "p", "open", "2026-01-01 00:00:00");
    let r = import_sql(&mut b, &sql).unwrap();
    assert_eq!(r.updated, 1);
    let (cnt, status, uid): (i64, String, Option<String>) = b
        .query_row(
            "SELECT count(*), max(status), max(uid) FROM plans",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(cnt, 1, "不应新建重复 plan");
    assert_eq!(status, "dropped");
    assert_eq!(uid.as_deref(), Some("mach-a:plan:1"), "应采纳快照 uid");
}

/// #498：重复导入幂等（uid 键去重），0 inserted / 0 updated。
#[test]
fn import_plan_idempotent() {
    let a = test_conn();
    seed_plan(
        &a,
        1,
        Some("mach-a:plan:1"),
        "p",
        "open",
        "2026-01-01 00:00:00",
    );
    let sql = crate::db::sync::export_sql(&a).unwrap();
    let mut b = test_conn();
    import_sql(&mut b, &sql).unwrap();
    let r = import_sql(&mut b, &sql).unwrap();
    assert_eq!((r.inserted, r.updated), (0, 0));
    let n: i64 = b
        .query_row("SELECT count(*) FROM plans", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 1);
}

/// #498：旧版快照无 uid 列（重放后为 NULL）——LWW 胜出也不抹掉本地稳定键。
#[test]
fn import_legacy_plan_snapshot_preserves_local_uid() {
    let sql = "-- mint sync snapshot v1 (mach-a)\n\
        INSERT INTO plans (id, title, status, created_at, updated_at) \
        VALUES (1, 'p', 'open', '2026-01-01 00:00:00', '2026-03-01 00:00:00');\n";
    let mut b = test_conn();
    seed_plan(
        &b,
        3,
        Some("mach-b:plan:3"),
        "p",
        "open",
        "2026-01-01 00:00:00",
    );
    let r = import_sql(&mut b, sql).unwrap();
    assert_eq!(r.updated, 1, "快照更新应 LWW 胜出");
    let (id, status, uid): (i64, String, Option<String>) = b
        .query_row("SELECT id, status, uid FROM plans", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .unwrap();
    assert_eq!(id, 3, "按 (title, milestone_id) 匹配本地 plan");
    assert_eq!(status, "open");
    assert_eq!(
        uid.as_deref(),
        Some("mach-b:plan:3"),
        "旧快照不得抹掉本地 uid"
    );
}

/// #497：手动 drop 标记随 LWW 传播（A 机 `plan drop` → C 机 manual_dropped=1）。
#[test]
fn import_plan_manual_dropped_propagates() {
    let a = test_conn();
    seed_plan(
        &a,
        1,
        Some("mach-a:plan:1"),
        "p",
        "dropped",
        "2026-02-01 00:00:00",
    );
    a.execute("UPDATE plans SET manual_dropped = 1 WHERE id = 1", [])
        .unwrap();
    let sql = crate::db::sync::export_sql(&a).unwrap();

    let mut b = test_conn();
    seed_plan(
        &b,
        3,
        Some("mach-a:plan:1"),
        "p",
        "open",
        "2026-01-01 00:00:00",
    );
    let r = import_sql(&mut b, &sql).unwrap();
    assert_eq!(r.updated, 1);
    let (status, manual): (String, Option<i64>) = b
        .query_row("SELECT status, manual_dropped FROM plans", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(status, "dropped");
    assert_eq!(manual, Some(1), "手动 drop 标记应随 LWW 传播");
}

/// #497：旧版快照无 manual_dropped 列（重放为 NULL）——LWW 胜出也不清零本地标记。
#[test]
fn import_legacy_plan_snapshot_preserves_manual_dropped() {
    let sql = "-- mint sync snapshot v1 (mach-a)\n\
        INSERT INTO plans (id, title, status, created_at, updated_at) \
        VALUES (1, 'p', 'dropped', '2026-01-01 00:00:00', '2026-03-01 00:00:00');\n";
    let mut b = test_conn();
    seed_plan(
        &b,
        3,
        Some("mach-b:plan:3"),
        "p",
        "dropped",
        "2026-01-01 00:00:00",
    );
    b.execute("UPDATE plans SET manual_dropped = 1 WHERE id = 3", [])
        .unwrap();
    let r = import_sql(&mut b, sql).unwrap();
    assert_eq!(r.updated, 1, "快照更新应 LWW 胜出");
    let manual: Option<i64> = b
        .query_row("SELECT manual_dropped FROM plans WHERE id = 3", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(manual, Some(1), "旧快照不得清零手动 drop 标记");
}
