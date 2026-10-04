//! 迁移：建表、幂等、版本升级与索引。

use super::*;

/// 迁移幂等：重复打开不报错，9 表齐全，user_version 正确。
#[test]
fn migrate_creates_tables_and_sets_version() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    migrate(&conn).unwrap(); // 幂等

    let version: i32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, CURRENT_VERSION);

    let tables: Vec<String> = conn
        .prepare(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE 'issues_fts_%'",
        )
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(tables.len(), 11);
    for t in [
        "projects",
        "issues",
        "labels",
        "issue_labels",
        "milestones",
        "plans",
        "machines",
        "milestone_direct_issues",
        "issue_links",
        "container_links",
        "issues_fts",
    ] {
        assert!(tables.iter().any(|n| n == t), "missing table {t}");
    }

    // 002 加列：issues.hit_count 存在
    let cols: Vec<String> = conn
        .prepare("PRAGMA table_info(issues)")
        .unwrap()
        .query_map([], |r| r.get(1))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(cols.iter().any(|c| c == "hit_count"), "missing hit_count");
    // 004 加列：issues.priority 存在
    assert!(cols.iter().any(|c| c == "priority"), "missing priority");
    // 002 加列：issues.machine_id/uid
    assert!(cols.iter().any(|c| c == "machine_id"), "missing machine_id");
    assert!(cols.iter().any(|c| c == "uid"), "missing uid");

    // 006/007/008 加列：plans.uid（跨机稳定键）+ plans.manual_dropped（手动终态标记）+ sort_order（显式排序）
    let pcols: Vec<String> = conn
        .prepare("PRAGMA table_info(plans)")
        .unwrap()
        .query_map([], |r| r.get(1))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(pcols.iter().any(|c| c == "uid"), "plans missing uid");
    assert!(
        pcols.iter().any(|c| c == "manual_dropped"),
        "plans missing manual_dropped"
    );
    assert!(
        pcols.iter().any(|c| c == "sort_order"),
        "plans missing sort_order"
    );
}

/// 既有 v1 库升级：migrate 从 user_version=1 自动跑 002/003（machines/列/color/FTS 扩展），不崩溃（#1 回归）。
#[test]
fn migrate_upgrades_v1_to_current() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    // 仅建 v1 schema（001）
    conn.execute_batch(MIGRATION_001).unwrap();
    migrate(&conn).unwrap();

    let version: i32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, CURRENT_VERSION, "v1 库应升级到当前版本");

    let tables: Vec<String> = conn
        .prepare(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE 'issues_fts_%'",
        )
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(
        tables.iter().any(|n| n == "machines"),
        "升级后应有 machines 表"
    );

    let cols: Vec<String> = conn
        .prepare("PRAGMA table_info(issues)")
        .unwrap()
        .query_map([], |r| r.get(1))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(
        cols.iter().any(|c| c == "machine_id"),
        "升级后应有 machine_id"
    );
    assert!(cols.iter().any(|c| c == "uid"), "升级后应有 uid");

    let lcols: Vec<String> = conn
        .prepare("PRAGMA table_info(labels)")
        .unwrap()
        .query_map([], |r| r.get(1))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(
        lcols.iter().any(|c| c == "color"),
        "升级后 labels 应有 color"
    );

    // FTS 扩展：虚表含六列。
    let fcols: Vec<String> = conn
        .prepare("PRAGMA table_info(issues_fts)")
        .unwrap()
        .query_map([], |r| r.get(1))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    for col in ["kind", "status", "priority", "labels"] {
        assert!(
            fcols.iter().any(|c| c == col),
            "升级后 issues_fts 应有 {col}"
        );
    }
}

/// 005 运行时热点索引：issues 过滤 + plans.milestone_id 索引齐全（#300）。
#[test]
fn runtime_indexes_created() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let issue_idx: Vec<String> = conn
        .prepare("PRAGMA index_list(issues)")
        .unwrap()
        .query_map([], |r| r.get(1))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    for name in [
        "idx_issues_status",
        "idx_issues_plan_id",
        "idx_issues_machine_id",
        "idx_issues_uid",
    ] {
        assert!(
            issue_idx.iter().any(|n| n == name),
            "issues 缺索引 {name}: {issue_idx:?}"
        );
    }
    let plan_idx: Vec<String> = conn
        .prepare("PRAGMA index_list(plans)")
        .unwrap()
        .query_map([], |r| r.get(1))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(
        plan_idx.iter().any(|n| n == "idx_plans_milestone_id"),
        "plans 缺索引 idx_plans_milestone_id: {plan_idx:?}"
    );
    assert!(
        plan_idx.iter().any(|n| n == "idx_plans_uid"),
        "plans 缺索引 idx_plans_uid: {plan_idx:?}"
    );
    assert!(
        plan_idx.iter().any(|n| n == "idx_plans_milestone_sort"),
        "plans 缺索引 idx_plans_milestone_sort: {plan_idx:?}"
    );

    // 009 建表 + 索引：container_links（容器级 blocks）
    let link_idx: Vec<String> = conn
        .prepare("PRAGMA index_list(container_links)")
        .unwrap()
        .query_map([], |r| r.get(1))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(
        link_idx.iter().any(|n| n == "idx_container_links_to"),
        "container_links 缺索引 idx_container_links_to: {link_idx:?}"
    );
    let link_cols: Vec<String> = conn
        .prepare("PRAGMA table_info(container_links)")
        .unwrap()
        .query_map([], |r| r.get(1))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    for c in ["kind", "from_id", "type", "to_id", "created_at"] {
        assert!(link_cols.iter().any(|n| n == c), "container_links 缺列 {c}");
    }
}

/// 既有 v2 库升级：migrate 从 user_version=2 自动跑 003（FTS 扩展），存量数据回填。
#[test]
fn migrate_upgrades_v2_to_current() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    // 建 v2 schema（001 + 002）
    conn.execute_batch(MIGRATION_001).unwrap();
    conn.execute_batch(MIGRATION_002).unwrap();
    // 造数据：issue + label + 关联。
    conn.execute("INSERT INTO projects (name) VALUES ('p')", [])
        .unwrap();
    conn.execute(
        "INSERT INTO issues (title, project_id) VALUES ('needle', 1)",
        [],
    )
    .unwrap();
    conn.execute("INSERT INTO labels (name) VALUES ('backend')", [])
        .unwrap();
    conn.execute(
        "INSERT INTO issue_labels (issue_id, label_id) VALUES (1, 1)",
        [],
    )
    .unwrap();
    // v2 回填（001 已建 FTS，需手动回填标题）。
    conn.execute(
        "INSERT INTO issues_fts(rowid, title, body) SELECT id, title, body FROM issues",
        [],
    )
    .unwrap();
    migrate(&conn).unwrap();

    let version: i32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, CURRENT_VERSION, "v2 库应升级到当前版本");
    // 存量 labels 可搜（回填子查询聚合）。
    let hit: i64 = conn
        .query_row(
            "SELECT count(*) FROM issues_fts WHERE issues_fts MATCH 'backend'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(hit, 1, "存量 label 可被 FTS 搜到");
}
