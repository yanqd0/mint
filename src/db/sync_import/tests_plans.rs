//! plans 合并（uid / manual_dropped / LWW）单测，自 `tests.rs` 外迁（>300 行规范）。

use rusqlite::{Connection, params};

use super::super::import_sql;
use super::test_conn;

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
