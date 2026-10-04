//! #104 唯一 running 写侧守卫：0→N 放行、净计数不变放行、增加即回滚、显式置位预检。

use super::*;
use crate::models::ContainerStatus;

/// 追加一个指定状态的 issue，返回 id（守卫测试需要多个分属不同 milestone 的 issue）。
fn add_issue_with(conn: &Connection, title: &str, status: &str) -> i64 {
    conn.execute(
        "INSERT INTO issues (title, status) VALUES (?1, ?2)",
        params![title, status],
    )
    .unwrap();
    conn.last_insert_rowid()
}

/// milestone 当前状态。
fn milestone_status(conn: &Connection, id: i64) -> ContainerStatus {
    get(conn, ContainerKind::Milestone, id)
        .unwrap()
        .unwrap()
        .status
}

/// 直挂某 milestone 的记录数（回滚断言用）。
fn direct_count(conn: &Connection, id: i64) -> i64 {
    conn.query_row(
        "SELECT count(*) FROM milestone_direct_issues WHERE milestone_id = ?1",
        params![id],
        |r| r.get(0),
    )
    .unwrap()
}

/// 0 → N：尚无 running 时允许挂入在途 issue（before 为空一律放行）。
#[test]
fn first_running_is_allowed() {
    let (conn, iid) = setup();
    set_status(&conn, iid, "dev");
    let ms = create(
        &conn,
        ContainerKind::Milestone,
        "a",
        Some("0.1.0"),
        None,
        None,
    )
    .unwrap();
    assert!(running_milestones(&conn).unwrap().is_empty());
    link_direct(&conn, ms, iid).unwrap();
    assert_eq!(milestone_status(&conn, ms), ContainerStatus::Running);
}

/// 已有 running 时，挂入在途 issue 让第二个 milestone 变 running → 拒绝并整体回滚。
#[test]
fn second_running_is_rejected_and_rolled_back() {
    let (conn, iid) = setup();
    set_status(&conn, iid, "dev");
    let ms1 = create(
        &conn,
        ContainerKind::Milestone,
        "a",
        Some("0.1.0"),
        None,
        None,
    )
    .unwrap();
    link_direct(&conn, ms1, iid).unwrap();
    let ms2 = create(
        &conn,
        ContainerKind::Milestone,
        "b",
        Some("0.2.0"),
        None,
        None,
    )
    .unwrap();
    let iid2 = add_issue_with(&conn, "b", "dev");

    let err = link_direct(&conn, ms2, iid2).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("refusing to add a running milestone"), "{msg}");
    assert!(msg.contains("#2 (0.2.0) would start"), "{msg}");
    assert!(msg.contains("already running: #1 (0.1.0)"), "{msg}");
    assert!(
        msg.contains("`mint milestone set 2 --status running --force`"),
        "{msg}"
    );

    // 回滚：新侧仍 open，且直挂行未落库。
    assert_eq!(milestone_status(&conn, ms2), ContainerStatus::Open);
    assert_eq!(direct_count(&conn, ms2), 0);
}

/// 跨桶移动净计数不变（A running → B open）→ 放行：只在「数量增加」时才拒。
#[test]
fn net_zero_move_is_allowed() {
    let (conn, iid) = setup();
    let ms1 = create(
        &conn,
        ContainerKind::Milestone,
        "a",
        Some("0.1.0"),
        None,
        None,
    )
    .unwrap();
    let ms2 = create(
        &conn,
        ContainerKind::Milestone,
        "b",
        Some("0.2.0"),
        None,
        None,
    )
    .unwrap();
    let pid = create(&conn, ContainerKind::Plan, "p", None, None, Some(ms1)).unwrap();
    set_issue_plan(&conn, iid, pid).unwrap();
    set_status(&conn, iid, "dev");
    sync_container_status(&conn, iid).unwrap();
    assert_eq!(milestone_status(&conn, ms1), ContainerStatus::Running);

    assert_eq!(move_plan(&conn, pid, ms2).unwrap(), 0, "dev 不重置");
    assert_eq!(milestone_status(&conn, ms1), ContainerStatus::Open);
    assert_eq!(milestone_status(&conn, ms2), ContainerStatus::Running);
}

/// 移动在途 plan 到 open milestone 且旧侧不回落（原无归属）→ 计数增加，拒绝并回滚归属。
#[test]
fn move_into_open_milestone_is_rejected() {
    let (conn, iid) = setup();
    // ms1 先 running：plan p1 + 在途 issue。
    let ms1 = create(
        &conn,
        ContainerKind::Milestone,
        "a",
        Some("0.1.0"),
        None,
        None,
    )
    .unwrap();
    let p1 = create(&conn, ContainerKind::Plan, "p1", None, None, Some(ms1)).unwrap();
    set_issue_plan(&conn, iid, p1).unwrap();
    set_status(&conn, iid, "dev");
    sync_container_status(&conn, iid).unwrap();
    assert_eq!(milestone_status(&conn, ms1), ContainerStatus::Running);

    // 无归属的在途 plan p2 + open milestone ms2。
    let iid2 = add_issue_with(&conn, "b", "dev");
    let p2 = create(&conn, ContainerKind::Plan, "p2", None, None, None).unwrap();
    set_issue_plan(&conn, iid2, p2).unwrap();
    sync_container_status(&conn, iid2).unwrap();
    let ms2 = create(
        &conn,
        ContainerKind::Milestone,
        "b",
        Some("0.2.0"),
        None,
        None,
    )
    .unwrap();

    let err = move_plan(&conn, p2, ms2).unwrap_err();
    assert!(
        err.to_string()
            .contains("refusing to add a running milestone"),
        "{err}"
    );
    // 回滚：归属未变、ms2 仍 open。
    assert_eq!(
        get(&conn, ContainerKind::Plan, p2)
            .unwrap()
            .unwrap()
            .milestone_id,
        None
    );
    assert_eq!(milestone_status(&conn, ms2), ContainerStatus::Open);
}

/// 显式置 running 预检：空放行、目标已 running 幂等放行、已有其他 running 拒绝（`--force` 才跳过）。
#[test]
fn start_allowed_precheck_cases() {
    let (conn, _) = setup();
    let ms1 = create(
        &conn,
        ContainerKind::Milestone,
        "a",
        Some("0.1.0"),
        None,
        None,
    )
    .unwrap();
    let ms2 = create(
        &conn,
        ContainerKind::Milestone,
        "b",
        Some("0.2.0"),
        None,
        None,
    )
    .unwrap();
    ensure_running_start_allowed(&conn, ms1).unwrap(); // 全 open：放行
    set_milestone_status(&conn, ms1, ContainerStatus::Running).unwrap();
    ensure_running_start_allowed(&conn, ms1).unwrap(); // 已 running：幂等放行
    let err = ensure_running_start_allowed(&conn, ms2).unwrap_err();
    assert!(
        err.to_string()
            .contains("`mint milestone set 2 --status running --force`"),
        "{err}"
    );
    ensure_running_start_allowed(&conn, 999).unwrap(); // 不存在：交给 not found 报错
}

/// 净计数减少（发布/取消）放行——守卫只看数量，不追具体 id。
#[test]
fn running_decrease_is_allowed() {
    let (conn, _) = setup();
    let ms1 = create(
        &conn,
        ContainerKind::Milestone,
        "a",
        Some("0.1.0"),
        None,
        None,
    )
    .unwrap();
    let ms2 = create(
        &conn,
        ContainerKind::Milestone,
        "b",
        Some("0.2.0"),
        None,
        None,
    )
    .unwrap();
    set_milestone_status(&conn, ms1, ContainerStatus::Running).unwrap();
    set_milestone_status(&conn, ms2, ContainerStatus::Running).unwrap();
    let before = running_milestones(&conn).unwrap();
    assert_eq!(before.len(), 2);
    set_milestone_status(&conn, ms2, ContainerStatus::Done).unwrap();
    ensure_running_not_increased(&conn, &before).unwrap();
}

/// version 为空（schema NOT NULL，仅手工 SQL 脏数据）时文案只写 `#N`，不留空括号。
#[test]
fn label_without_version_has_no_empty_parens() {
    let (conn, iid) = setup();
    set_status(&conn, iid, "dev");
    let legacy = create(
        &conn,
        ContainerKind::Milestone,
        "legacy",
        Some("legacy-0"),
        None,
        None,
    )
    .unwrap();
    link_direct(&conn, legacy, iid).unwrap();
    // 清空 version（NOT NULL 允许空串；CLI 层拒绝空值，只有脏数据会出现）。
    conn.execute(
        "UPDATE milestones SET version = '' WHERE id = ?1",
        params![legacy],
    )
    .unwrap();

    let iid2 = add_issue_with(&conn, "b", "dev");
    let ms2 = create(
        &conn,
        ContainerKind::Milestone,
        "b",
        Some("0.2.0"),
        None,
        None,
    )
    .unwrap();
    let err = link_direct(&conn, ms2, iid2).unwrap_err();
    assert!(err.to_string().contains("already running: #1)"), "{err}");
}
