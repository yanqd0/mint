//! 归属挂载/解绑与状态级联。

use super::*;
use crate::models::ContainerStatus;

/// milestone 直接挂 issue（无 plan）+ 派生状态同步。
#[test]
fn milestone_direct_issue_derives_status() {
    let (conn, iid) = setup();
    let rid = create(
        &conn,
        ContainerKind::Milestone,
        "r",
        Some("0.1.0"),
        None,
        None,
    )
    .unwrap();
    link_direct(&conn, rid, iid).unwrap();
    assert_eq!(
        get(&conn, ContainerKind::Milestone, rid)
            .unwrap()
            .unwrap()
            .status,
        ContainerStatus::Open
    );

    set_status(&conn, iid, "done");
    sync_container_status(&conn, iid).unwrap();
    // milestone 不随子项全部完成自动 done：派生 done → running（版本进行中待发布）。
    assert_eq!(
        get(&conn, ContainerKind::Milestone, rid)
            .unwrap()
            .unwrap()
            .status,
        ContainerStatus::Running
    );
}

/// 手动 done（发布）不被后续派生覆盖。
#[test]
fn milestone_manual_done_not_overwritten_by_sync() {
    let (conn, iid) = setup();
    let rid = create(
        &conn,
        ContainerKind::Milestone,
        "r",
        Some("0.1.0"),
        None,
        None,
    )
    .unwrap();
    link_direct(&conn, rid, iid).unwrap();
    set_milestone_status(&conn, rid, ContainerStatus::Done).unwrap();
    // 子项状态变化触发 sync → milestone 保持手动 done。
    set_status(&conn, iid, "dev");
    sync_container_status(&conn, iid).unwrap();
    assert_eq!(
        get(&conn, ContainerKind::Milestone, rid)
            .unwrap()
            .unwrap()
            .status,
        ContainerStatus::Done
    );
}

/// 转移 plan 后源容器状态回落（源端重算，不再残留 running）。
#[test]
fn set_issue_plan_recomputes_source_plan() {
    let (conn, iid) = setup();
    let pid_a = create(&conn, ContainerKind::Plan, "a", None, None, None).unwrap();
    let pid_b = create(&conn, ContainerKind::Plan, "b", None, None, None).unwrap();
    set_issue_plan(&conn, iid, pid_a).unwrap();
    set_status(&conn, iid, "done");
    sync_container_status(&conn, iid).unwrap();
    assert_eq!(
        get(&conn, ContainerKind::Plan, pid_a)
            .unwrap()
            .unwrap()
            .status,
        ContainerStatus::Done
    );
    // 移到 plan B → A 无 issue 应回落 open，B 全 done
    set_issue_plan(&conn, iid, pid_b).unwrap();
    assert_eq!(
        get(&conn, ContainerKind::Plan, pid_a)
            .unwrap()
            .unwrap()
            .status,
        ContainerStatus::Open
    );
    assert_eq!(
        get(&conn, ContainerKind::Plan, pid_b)
            .unwrap()
            .unwrap()
            .status,
        ContainerStatus::Done
    );
}

/// 解绑直属 milestone 后源 milestone 状态回落。
#[test]
fn unlink_direct_recomputes_source_milestone() {
    let (conn, iid) = setup();
    let rid = create(
        &conn,
        ContainerKind::Milestone,
        "r",
        Some("0.1.0"),
        None,
        None,
    )
    .unwrap();
    link_direct(&conn, rid, iid).unwrap();
    set_status(&conn, iid, "done");
    sync_container_status(&conn, iid).unwrap();
    assert_eq!(
        get(&conn, ContainerKind::Milestone, rid)
            .unwrap()
            .unwrap()
            .status,
        ContainerStatus::Running
    );
    unlink_direct(&conn, rid, iid).unwrap();
    assert_eq!(
        get(&conn, ContainerKind::Milestone, rid)
            .unwrap()
            .unwrap()
            .status,
        ContainerStatus::Open
    );
}

/// detach 不存在的 milestone 报 not found（与 attach 校验对齐）。
#[test]
fn unlink_direct_missing_milestone_errors() {
    let (conn, iid) = setup();
    let err = unlink_direct(&conn, 999, iid).unwrap_err();
    assert!(
        err.to_string().contains("milestone #999 not found"),
        "err: {err}"
    );
}

/// detach 不存在的 issue 报 not found（#341：此前静默报成功）。
#[test]
fn unlink_direct_missing_issue_errors() {
    let (conn, _iid) = setup();
    let rid = create(
        &conn,
        ContainerKind::Milestone,
        "r",
        Some("0.1.0"),
        None,
        None,
    )
    .unwrap();
    let err = unlink_direct(&conn, rid, 999).unwrap_err();
    assert!(
        err.to_string().contains("issue #999 not found"),
        "err: {err}"
    );
}

/// unset_issue_plan 不存在的 issue 报 not found（#341）。
#[test]
fn unset_issue_plan_missing_issue_errors() {
    let (conn, _iid) = setup();
    let err = unset_issue_plan(&conn, 999).unwrap_err();
    assert!(
        err.to_string().contains("issue #999 not found"),
        "err: {err}"
    );
}

/// 二选一：issue 挂 plan 后不能再直接挂 milestone。
#[test]
fn plan_issue_cannot_direct_link() {
    let (conn, iid) = setup();
    let pid = create(&conn, ContainerKind::Plan, "p", None, None, None).unwrap();
    set_issue_plan(&conn, iid, pid).unwrap();
    let rid = create(
        &conn,
        ContainerKind::Milestone,
        "r",
        Some("0.1.0"),
        None,
        None,
    )
    .unwrap();
    let err = link_direct(&conn, rid, iid).unwrap_err();
    assert!(err.to_string().contains("already belongs to a plan"));
}

/// plan → milestone 级联：issue 变更同步 plan，再同步 milestone。
#[test]
fn issue_plan_milestone_cascade() {
    let (conn, iid) = setup();
    let rid = create(
        &conn,
        ContainerKind::Milestone,
        "r",
        Some("0.1.0"),
        None,
        None,
    )
    .unwrap();
    let pid = create(&conn, ContainerKind::Plan, "p", None, None, Some(rid)).unwrap();
    set_issue_plan(&conn, iid, pid).unwrap();

    set_status(&conn, iid, "done");
    sync_container_status(&conn, iid).unwrap();
    assert_eq!(
        get(&conn, ContainerKind::Plan, pid)
            .unwrap()
            .unwrap()
            .status,
        ContainerStatus::Done
    );
    assert_eq!(
        get(&conn, ContainerKind::Milestone, rid)
            .unwrap()
            .unwrap()
            .status,
        ContainerStatus::Running
    );
}
