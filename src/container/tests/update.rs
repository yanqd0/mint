//! plan/milestone 字段更新。

use super::*;

/// update_plan 更新 title，body 不变。
#[test]
fn update_plan_changes_title() {
    let (conn, _) = setup();
    let pid = create(
        &conn,
        ContainerKind::Plan,
        "old",
        None,
        Some("old body"),
        None,
    )
    .unwrap();
    update_plan(&conn, pid, Some("new title"), None).unwrap();
    let p = get(&conn, ContainerKind::Plan, pid).unwrap().unwrap();
    assert_eq!(p.title, "new title");
    assert_eq!(p.body.as_deref(), Some("old body"));
}

/// update_plan 更新 body，title 不变。
#[test]
fn update_plan_preserves_title() {
    let (conn, _) = setup();
    let pid = create(
        &conn,
        ContainerKind::Plan,
        "t",
        None,
        Some("old body"),
        None,
    )
    .unwrap();
    update_plan(&conn, pid, None, Some("new body")).unwrap();
    let p = get(&conn, ContainerKind::Plan, pid).unwrap().unwrap();
    assert_eq!(p.title, "t");
    assert_eq!(p.body.as_deref(), Some("new body"));
}

/// update_plan 不存在的 plan 报 not found。
#[test]
fn update_plan_missing_errors() {
    let (conn, _) = setup();
    let err = update_plan(&conn, 999, Some("x"), None).unwrap_err();
    assert!(err.to_string().contains("plan #999 not found"), "{err}");
}

/// update_milestone 更新 version，title/body 不变。
#[test]
fn update_milestone_changes_version() {
    let (conn, _) = setup();
    let rid = create(
        &conn,
        ContainerKind::Milestone,
        "r",
        Some("0.1.0"),
        Some("old body"),
        None,
    )
    .unwrap();
    update_milestone(&conn, rid, None, Some("0.2.0"), None).unwrap();
    let r = get(&conn, ContainerKind::Milestone, rid).unwrap().unwrap();
    assert_eq!(r.version, Some("0.2.0".to_string()));
    assert_eq!(r.title, "r");
    assert_eq!(r.body.as_deref(), Some("old body"));
}

/// update_milestone 不存在的 milestone 报 not found。
#[test]
fn update_milestone_missing_errors() {
    let (conn, _) = setup();
    let err = update_milestone(&conn, 999, Some("x"), None, None).unwrap_err();
    assert!(
        err.to_string().contains("milestone #999 not found"),
        "{err}"
    );
}
