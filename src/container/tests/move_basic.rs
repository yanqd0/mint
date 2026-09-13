//! 跨 milestone 移动的基础语义与排期重置。

use super::*;
use crate::models::ContainerStatus;

/// #223 修复：跨 milestone 移动 plan 时其下 planned issue 重置回 open（排期作废），
/// plan 不再派生 running，旧侧派生回落、新侧按现状推进。
#[test]
fn move_plan_resets_planned_issues_and_derives() {
    let (conn, iid) = setup();
    let ms_a = create(
        &conn,
        ContainerKind::Milestone,
        "a",
        Some("0.5.0"),
        None,
        None,
    )
    .unwrap();
    let ms_b = create(
        &conn,
        ContainerKind::Milestone,
        "b",
        Some("2.0.0"),
        None,
        None,
    )
    .unwrap();
    let pid = create(&conn, ContainerKind::Plan, "p", None, None, Some(ms_a)).unwrap();
    set_issue_plan(&conn, iid, pid).unwrap();
    set_status(&conn, iid, "planned");
    sync_container_status(&conn, iid).unwrap();
    // 初始：issue planned → plan running → msA running（#223 现象）。
    assert_eq!(
        get(&conn, ContainerKind::Plan, pid)
            .unwrap()
            .unwrap()
            .status,
        ContainerStatus::Running
    );
    assert_eq!(
        get(&conn, ContainerKind::Milestone, ms_a)
            .unwrap()
            .unwrap()
            .status,
        ContainerStatus::Running
    );
    // 移到 msB（未来版本桶）：planned 重置 open，返回 1。
    let reset = move_plan(&conn, pid, ms_b).unwrap();
    assert_eq!(reset, 1);
    let st: String = conn
        .query_row(
            "SELECT status FROM issues WHERE id = ?1",
            params![iid],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(st, "open");
    // plan 回落 open；旧侧 msA 回落 open；新侧 msB 派生 open（plan 全 open）。
    assert_eq!(
        get(&conn, ContainerKind::Plan, pid)
            .unwrap()
            .unwrap()
            .status,
        ContainerStatus::Open
    );
    assert_eq!(
        get(&conn, ContainerKind::Milestone, ms_a)
            .unwrap()
            .unwrap()
            .status,
        ContainerStatus::Open
    );
    assert_eq!(
        get(&conn, ContainerKind::Milestone, ms_b)
            .unwrap()
            .unwrap()
            .status,
        ContainerStatus::Open
    );
}

/// 只重置 planned：dev/test/done/dropped 保持，reset 计数精确。
#[test]
fn move_plan_keeps_non_planned_issues() {
    let (conn, iid) = setup();
    let ms_a = create(
        &conn,
        ContainerKind::Milestone,
        "a",
        Some("0.5.0"),
        None,
        None,
    )
    .unwrap();
    let ms_b = create(
        &conn,
        ContainerKind::Milestone,
        "b",
        Some("2.0.0"),
        None,
        None,
    )
    .unwrap();
    let pid = create(&conn, ContainerKind::Plan, "p", None, None, Some(ms_a)).unwrap();
    set_issue_plan(&conn, iid, pid).unwrap();
    set_status(&conn, iid, "planned");
    // 追加 dev/test/done/dropped 4 个 issue 挂同一 plan。
    for (t, st) in [("d", "dev"), ("t", "test"), ("n", "done"), ("r", "dropped")] {
        conn.execute("INSERT INTO issues (title) VALUES (?1)", params![t])
            .unwrap();
        let id = conn.last_insert_rowid();
        set_issue_plan(&conn, id, pid).unwrap();
        set_status(&conn, id, st);
    }
    let reset = move_plan(&conn, pid, ms_b).unwrap();
    assert_eq!(reset, 1, "仅 planned 被重置");
    let statuses: Vec<String> = conn
        .prepare("SELECT status FROM issues WHERE plan_id = ?1")
        .unwrap()
        .query_map(params![pid], |r| r.get(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        statuses,
        vec!["open", "dev", "test", "done", "dropped"],
        "planned→open，其余保持"
    );
}

/// #446：手动 drop 的空 plan 跨 milestone 移动后不被派生复活（仍 dropped），
/// 两侧 milestone 照常重算（旧侧回落 open、新侧纳入该 plan）。
#[test]
fn move_plan_keeps_manually_dropped_empty_plan() {
    let (conn, _) = setup();
    let ms_a = create(
        &conn,
        ContainerKind::Milestone,
        "a",
        Some("0.5.0"),
        None,
        None,
    )
    .unwrap();
    let ms_b = create(
        &conn,
        ContainerKind::Milestone,
        "b",
        Some("0.8.0"),
        None,
        None,
    )
    .unwrap();
    let pid = create(&conn, ContainerKind::Plan, "p", None, None, Some(ms_a)).unwrap();
    set_plan_status(&conn, pid, ContainerStatus::Dropped).unwrap();
    let reset = move_plan(&conn, pid, ms_b).unwrap();
    assert_eq!(reset, 0, "空 plan 无 planned issue 可重置");
    assert_eq!(
        get(&conn, ContainerKind::Plan, pid)
            .unwrap()
            .unwrap()
            .status,
        ContainerStatus::Dropped,
        "手动 drop 的空 plan 不被派生覆盖（#446）"
    );
    assert_eq!(
        get(&conn, ContainerKind::Milestone, ms_a)
            .unwrap()
            .unwrap()
            .status,
        ContainerStatus::Open,
        "旧侧回落 open"
    );
    // 新侧纳入该 plan：子项全 dropped 派生 dropped → milestone 版本桶映射为 running。
    assert_eq!(
        get(&conn, ContainerKind::Milestone, ms_b)
            .unwrap()
            .unwrap()
            .status,
        ContainerStatus::Running,
        "新侧同步（派生 dropped → running）"
    );
}

/// #446 反向保护：非空 plan 的 dropped 是派生结果，其下 issue 重开后必须重算，
/// 不被手动 dropped 守卫误锁死。
#[test]
fn derived_dropped_plan_rederives_on_reopen() {
    let (conn, iid) = setup();
    let pid = create(&conn, ContainerKind::Plan, "p", None, None, None).unwrap();
    set_issue_plan(&conn, iid, pid).unwrap();
    set_status(&conn, iid, "dropped");
    sync_container_status(&conn, iid).unwrap();
    assert_eq!(
        get(&conn, ContainerKind::Plan, pid)
            .unwrap()
            .unwrap()
            .status,
        ContainerStatus::Dropped,
        "全 dropped → 派生 dropped"
    );
    set_status(&conn, iid, "open");
    sync_container_status(&conn, iid).unwrap();
    assert_eq!(
        get(&conn, ContainerKind::Plan, pid)
            .unwrap()
            .unwrap()
            .status,
        ContainerStatus::Open,
        "issue 重开 → plan 重算（派生 dropped 不锁死）"
    );
}
