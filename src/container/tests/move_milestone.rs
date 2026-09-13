//! 跨 milestone 移动的边界与两侧 milestone 同步。

use super::*;
use crate::models::ContainerStatus;

/// 同 milestone 迁移 no-op：不重置排期。
#[test]
fn move_plan_same_milestone_is_noop() {
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
    let pid = create(&conn, ContainerKind::Plan, "p", None, None, Some(ms_a)).unwrap();
    set_issue_plan(&conn, iid, pid).unwrap();
    set_status(&conn, iid, "planned");
    sync_container_status(&conn, iid).unwrap();
    let reset = move_plan(&conn, pid, ms_a).unwrap();
    assert_eq!(reset, 0);
    let st: String = conn
        .query_row(
            "SELECT status FROM issues WHERE id = ?1",
            params![iid],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(st, "planned");
}

/// 移动 plan 到另一 milestone：两侧派生状态重算（旧侧回落、新侧推进），同一事务内原子。
#[test]
fn move_plan_syncs_both_milestones() {
    let (conn, iid) = setup();
    let aid = create(
        &conn,
        ContainerKind::Milestone,
        "a",
        Some("0.1.0"),
        None,
        None,
    )
    .unwrap();
    let bid = create(
        &conn,
        ContainerKind::Milestone,
        "b",
        Some("0.2.0"),
        None,
        None,
    )
    .unwrap();
    let pid = create(&conn, ContainerKind::Plan, "p", None, None, Some(aid)).unwrap();
    set_issue_plan(&conn, iid, pid).unwrap();
    set_status(&conn, iid, "done");
    sync_container_status(&conn, iid).unwrap();
    // plan 在 a 下且含 done issue → a 为 Running。
    assert_eq!(
        get(&conn, ContainerKind::Milestone, aid)
            .unwrap()
            .unwrap()
            .status,
        ContainerStatus::Running
    );
    // 移到 b → a 回落 Open、b 推进 Running、plan 归属更新。
    move_plan(&conn, pid, bid).unwrap();
    assert_eq!(
        get(&conn, ContainerKind::Milestone, aid)
            .unwrap()
            .unwrap()
            .status,
        ContainerStatus::Open
    );
    assert_eq!(
        get(&conn, ContainerKind::Milestone, bid)
            .unwrap()
            .unwrap()
            .status,
        ContainerStatus::Running
    );
    assert_eq!(
        get(&conn, ContainerKind::Plan, pid)
            .unwrap()
            .unwrap()
            .milestone_id,
        Some(bid)
    );
}

/// 移到空 milestone：新侧无子项 → Open。
#[test]
fn move_plan_to_empty_milestone() {
    let (conn, _) = setup();
    let aid = create(
        &conn,
        ContainerKind::Milestone,
        "a",
        Some("0.1.0"),
        None,
        None,
    )
    .unwrap();
    let bid = create(
        &conn,
        ContainerKind::Milestone,
        "b",
        Some("0.2.0"),
        None,
        None,
    )
    .unwrap();
    let pid = create(&conn, ContainerKind::Plan, "p", None, None, Some(aid)).unwrap();
    move_plan(&conn, pid, bid).unwrap();
    assert_eq!(
        get(&conn, ContainerKind::Milestone, bid)
            .unwrap()
            .unwrap()
            .status,
        ContainerStatus::Open
    );
}

/// plan 不存在 → 报错。
#[test]
fn move_plan_not_found() {
    let (conn, _) = setup();
    let bid = create(
        &conn,
        ContainerKind::Milestone,
        "b",
        Some("0.2.0"),
        None,
        None,
    )
    .unwrap();
    let err = move_plan(&conn, 999, bid).unwrap_err();
    assert!(err.to_string().contains("plan #999 not found"), "{err}");
}

/// 目标 milestone 不存在 → 报错。
#[test]
fn move_plan_missing_milestone() {
    let (conn, _) = setup();
    let aid = create(
        &conn,
        ContainerKind::Milestone,
        "a",
        Some("0.1.0"),
        None,
        None,
    )
    .unwrap();
    let pid = create(&conn, ContainerKind::Plan, "p", None, None, Some(aid)).unwrap();
    let err = move_plan(&conn, pid, 999).unwrap_err();
    assert!(
        err.to_string().contains("milestone #999 not found"),
        "{err}"
    );
}

/// 同 milestone 迁移：no-op，状态不变。
#[test]
fn move_plan_same_milestone_noop() {
    let (conn, _) = setup();
    let aid = create(
        &conn,
        ContainerKind::Milestone,
        "a",
        Some("0.1.0"),
        None,
        None,
    )
    .unwrap();
    let pid = create(&conn, ContainerKind::Plan, "p", None, None, Some(aid)).unwrap();
    move_plan(&conn, pid, aid).unwrap();
    assert_eq!(
        get(&conn, ContainerKind::Milestone, aid)
            .unwrap()
            .unwrap()
            .status,
        ContainerStatus::Open
    );
}
