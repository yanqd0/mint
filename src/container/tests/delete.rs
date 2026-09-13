//! 删除 plan/milestone/issue 的关联解绑与状态同步。

use super::*;
use crate::models::ContainerStatus;

/// 删除 plan：解绑其下 issue（plan_id 置 NULL），plan 消失、issue 保留。
#[test]
fn delete_plan_detaches_issues() {
    let (conn, iid) = setup();
    let pid = create(&conn, ContainerKind::Plan, "p", None, None, None).unwrap();
    set_issue_plan(&conn, iid, pid).unwrap();
    delete_plan(&conn, pid).unwrap();
    assert!(get(&conn, ContainerKind::Plan, pid).unwrap().is_none());
    let plan_id: Option<i64> = conn
        .query_row(
            "SELECT plan_id FROM issues WHERE id = ?1",
            params![iid],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(plan_id, None);
}

/// 删除 milestone：直接挂 issue 解绑、其下 plan 保留（milestone_id 置 NULL）、milestone 消失。
#[test]
fn delete_milestone_detaches_plan_and_direct() {
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
    link_direct(&conn, rid, iid).unwrap();
    delete_milestone(&conn, rid).unwrap();
    assert!(get(&conn, ContainerKind::Milestone, rid).unwrap().is_none());
    let p = get(&conn, ContainerKind::Plan, pid).unwrap().unwrap();
    assert_eq!(p.milestone_id, None);
    let cnt: i64 = conn
        .query_row("SELECT COUNT(*) FROM milestone_direct_issues", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(cnt, 0);
}

/// 删除不存在对象报 not found。
#[test]
fn delete_missing_errors() {
    let (conn, _) = setup();
    let err = delete_plan(&conn, 999).unwrap_err();
    assert!(err.to_string().contains("plan #999 not found"), "{err}");
    let err = delete_milestone(&conn, 999).unwrap_err();
    assert!(
        err.to_string().contains("milestone #999 not found"),
        "{err}"
    );
    let err = delete_issue(&conn, 999).unwrap_err();
    assert!(err.to_string().contains("issue #999 not found"), "{err}");
}

/// 删除 plan 后，其上级 milestone 派生状态回落（plan 不再参与派生）。
#[test]
fn delete_plan_syncs_milestone_status() {
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
        get(&conn, ContainerKind::Milestone, rid)
            .unwrap()
            .unwrap()
            .status,
        ContainerStatus::Running
    );
    delete_plan(&conn, pid).unwrap();
    assert_eq!(
        get(&conn, ContainerKind::Milestone, rid)
            .unwrap()
            .unwrap()
            .status,
        ContainerStatus::Open
    );
}

/// 物理删除属容器的 issue 后，父容器派生状态回落（done → open）。
#[test]
fn delete_issue_syncs_plan_status() {
    let (conn, iid) = setup();
    let pid = create(&conn, ContainerKind::Plan, "p", None, None, None).unwrap();
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
    delete_issue(&conn, iid).unwrap();
    assert_eq!(
        get(&conn, ContainerKind::Plan, pid)
            .unwrap()
            .unwrap()
            .status,
        ContainerStatus::Open
    );
}
