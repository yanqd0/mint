//! #496 直挂 milestone 的写侧守卫与读值确定性（自 `affiliation.rs` 外迁）。

use super::*;

/// #496：直挂至多一个 milestone——第二条不同直挂被拒；同一条重复挂载仍幂等。
#[test]
fn direct_link_rejects_second_milestone() {
    let (conn, iid) = setup();
    let r1 = create(
        &conn,
        ContainerKind::Milestone,
        "r1",
        Some("0.1.0"),
        None,
        None,
    )
    .unwrap();
    let r2 = create(
        &conn,
        ContainerKind::Milestone,
        "r2",
        Some("0.2.0"),
        None,
        None,
    )
    .unwrap();
    link_direct(&conn, r1, iid).unwrap();
    let err = link_direct(&conn, r2, iid).unwrap_err();
    assert!(
        err.to_string().contains("already belongs to milestone #1"),
        "err: {err}"
    );
    link_direct(&conn, r1, iid).unwrap(); // 同一条幂等
    let rows: i64 = conn
        .query_row(
            "SELECT count(*) FROM milestone_direct_issues WHERE issue_id = ?1",
            params![iid],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(rows, 1, "直挂应只有一条");
}

/// #496：历史重复直挂（绕过写侧守卫）读值确定——取最小 milestone_id，两个查询一致。
#[test]
fn effective_milestone_deterministic_with_duplicate_links() {
    let (conn, iid) = setup();
    let r1 = create(
        &conn,
        ContainerKind::Milestone,
        "r1",
        Some("0.1.0"),
        None,
        None,
    )
    .unwrap();
    let r2 = create(
        &conn,
        ContainerKind::Milestone,
        "r2",
        Some("0.2.0"),
        None,
        None,
    )
    .unwrap();
    // 直接写库模拟历史脏数据（写侧已不允许）。
    for rid in [r2, r1] {
        conn.execute(
            "INSERT INTO milestone_direct_issues (milestone_id, issue_id) VALUES (?1, ?2)",
            params![rid, iid],
        )
        .unwrap();
    }
    let one: Option<i64> = conn
        .query_row(db::ISSUE_EFFECTIVE_MILESTONE, params![iid], |r| r.get(0))
        .unwrap();
    let all: Option<i64> = conn
        .prepare(db::ISSUE_EFFECTIVE_MILESTONES)
        .unwrap()
        .query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, Option<i64>>(1)?))
        })
        .unwrap()
        .filter_map(Result::ok)
        .find(|(id, _)| *id == iid)
        .and_then(|(_, m)| m);
    assert_eq!(one, Some(1), "单条查询取最小 milestone_id");
    assert_eq!(all, Some(1), "全量查询取最小 milestone_id");
}
