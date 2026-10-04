//! 容器级链接（`container/link.rs`）单测。

use super::*;
use crate::db;
use rstest::rstest;

/// 建内存库 + 同 kind 的两个容器，返回 (conn, kind, a, b)。
fn setup(kind: ContainerKind) -> (Connection, ContainerKind, i64, i64) {
    let conn = db::open(std::path::Path::new(":memory:")).unwrap();
    match kind {
        ContainerKind::Plan => {
            conn.execute("INSERT INTO plans (title) VALUES ('a')", [])
                .unwrap();
            conn.execute("INSERT INTO plans (title) VALUES ('b')", [])
                .unwrap();
        }
        ContainerKind::Milestone => {
            conn.execute(
                "INSERT INTO milestones (title, version) VALUES ('a', '0.1.0')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO milestones (title, version) VALUES ('b', '0.2.0')",
                [],
            )
            .unwrap();
        }
    }
    let table = match kind {
        ContainerKind::Plan => "plans",
        ContainerKind::Milestone => "milestones",
    };
    let a: i64 = conn
        .query_row(
            &format!("SELECT id FROM {table} WHERE title='a'"),
            [],
            |r| r.get(0),
        )
        .unwrap();
    let b: i64 = conn
        .query_row(
            &format!("SELECT id FROM {table} WHERE title='b'"),
            [],
            |r| r.get(0),
        )
        .unwrap();
    (conn, kind, a, b)
}

/// 建内存库 + plan #1/#2 与 milestone #1（容器 id 空间各自独立），
/// 返回 (conn, plan1, plan2, milestone1)。
fn setup_both_kinds() -> (Connection, i64, i64, i64) {
    let conn = db::open(std::path::Path::new(":memory:")).unwrap();
    conn.execute("INSERT INTO plans (title) VALUES ('p1')", [])
        .unwrap();
    conn.execute("INSERT INTO plans (title) VALUES ('p2')", [])
        .unwrap();
    conn.execute(
        "INSERT INTO milestones (title, version) VALUES ('m1', '0.1.0')",
        [],
    )
    .unwrap();
    let plan1: i64 = conn
        .query_row("SELECT id FROM plans WHERE title='p1'", [], |r| r.get(0))
        .unwrap();
    let plan2: i64 = conn
        .query_row("SELECT id FROM plans WHERE title='p2'", [], |r| r.get(0))
        .unwrap();
    let ms1: i64 = conn
        .query_row("SELECT id FROM milestones WHERE title='m1'", [], |r| {
            r.get(0)
        })
        .unwrap();
    (conn, plan1, plan2, ms1)
}

/// kind 参数化：create 后 links_for 出向/入向 rel 与标题正确。
#[rstest]
#[case(ContainerKind::Plan)]
#[case(ContainerKind::Milestone)]
fn create_links_for_roundtrip(#[case] kind: ContainerKind) {
    let (conn, kind, a, b) = setup(kind);
    create(&conn, kind, a, ContainerLinkType::Blocks, b).unwrap();

    let la = links_for(&conn, kind, a).unwrap();
    assert_eq!(la.len(), 1);
    assert_eq!(la[0].other_id, b);
    assert_eq!(la[0].rel, "blocks");
    assert_eq!(la[0].other_title, "b");

    let lb = links_for(&conn, kind, b).unwrap();
    assert_eq!(lb.len(), 1);
    assert_eq!(lb[0].other_id, a);
    assert_eq!(lb[0].rel, "blocked_by");
    assert_eq!(lb[0].other_title, "a");
}

/// `blocked_by` 归一化：A blocked_by B 存 (B, blocks, A)，两侧视角正确。
#[test]
fn blocked_by_normalizes_direction() {
    let (conn, kind, a, b) = setup(ContainerKind::Plan);
    create(&conn, kind, a, ContainerLinkType::BlockedBy, b).unwrap();

    let stored: (i64, String, i64) = conn
        .query_row(
            "SELECT from_id, type, to_id FROM container_links",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(stored, (b, "blocks".to_string(), a));

    assert_eq!(links_for(&conn, kind, a).unwrap()[0].rel, "blocked_by");
    assert_eq!(links_for(&conn, kind, b).unwrap()[0].rel, "blocks");
}

/// 同向重复幂等（含 blocked_by 归一化后同向）；反向同类型互斥报错。
#[rstest]
#[case(ContainerKind::Plan, "plan")]
#[case(ContainerKind::Milestone, "milestone")]
fn create_idempotent_and_reverse_conflict(#[case] kind: ContainerKind, #[case] noun: &str) {
    let (conn, kind, a, b) = setup(kind);
    create(&conn, kind, a, ContainerLinkType::Blocks, b).unwrap();
    create(&conn, kind, a, ContainerLinkType::Blocks, b).unwrap();
    // B blocked_by A 归一化为 (A, blocks, B) → 与已存同向，幂等 no-op。
    create(&conn, kind, b, ContainerLinkType::BlockedBy, a).unwrap();
    let cnt: i64 = conn
        .query_row("SELECT COUNT(*) FROM container_links", [], |r| r.get(0))
        .unwrap();
    assert_eq!(cnt, 1);

    // 反向同类型 = 互相阻塞 → 互斥报错。
    let err = create(&conn, kind, b, ContainerLinkType::Blocks, a).unwrap_err();
    assert_eq!(
        err.to_string(),
        format!("{noun} #{b} already linked to #{a} as 'blocks'")
    );
    // A blocked_by B（= B blocks A）与已存的 A blocks B 亦互斥。
    let err = create(&conn, kind, a, ContainerLinkType::BlockedBy, b).unwrap_err();
    assert_eq!(
        err.to_string(),
        format!("{noun} #{a} already linked to #{b} as 'blocks'")
    );
}

/// 自环 / 端点不存在（kind 参数化文案）。
#[rstest]
#[case(ContainerKind::Plan, "plan")]
#[case(ContainerKind::Milestone, "milestone")]
fn create_rejects_self_and_missing(#[case] kind: ContainerKind, #[case] noun: &str) {
    let (conn, kind, a, _b) = setup(kind);
    let err = create(&conn, kind, a, ContainerLinkType::Blocks, a).unwrap_err();
    assert_eq!(
        err.to_string(),
        format!("cannot link {noun} #{a} to itself")
    );

    let err = create(&conn, kind, a, ContainerLinkType::Blocks, 999).unwrap_err();
    assert_eq!(err.to_string(), format!("{noun} #999 not found"));
}

/// kind 隔离：plan 与 milestone 各自 id 空间独立，链接与视角互不串台。
#[test]
fn create_kind_isolation_same_ids() {
    let (conn, plan1, plan2, ms1) = setup_both_kinds();
    assert_eq!((plan1, plan2, ms1), (1, 2, 1));
    // plan 侧建链：milestone 视角（同号 id）看不到。
    create(
        &conn,
        ContainerKind::Plan,
        plan1,
        ContainerLinkType::Blocks,
        plan2,
    )
    .unwrap();
    assert_eq!(
        links_for(&conn, ContainerKind::Plan, plan1).unwrap().len(),
        1
    );
    assert!(
        links_for(&conn, ContainerKind::Milestone, ms1)
            .unwrap()
            .is_empty()
    );
    assert!(
        links_for_all(&conn, ContainerKind::Milestone)
            .unwrap()
            .is_empty()
    );
    // 同号端点：plan #2 存在但 milestone #2 不存在 → 报 milestone 命名空间。
    let err = create(
        &conn,
        ContainerKind::Milestone,
        ms1,
        ContainerLinkType::Blocks,
        plan2,
    )
    .unwrap_err();
    assert_eq!(err.to_string(), "milestone #2 not found");
    // plan #3 不存在 → 报 plan 命名空间。
    let err = create(
        &conn,
        ContainerKind::Plan,
        plan1,
        ContainerLinkType::Blocks,
        3,
    )
    .unwrap_err();
    assert_eq!(err.to_string(), "plan #3 not found");
}

/// remove：正反向表述均可删；不存在的链接静默 no-op。
#[rstest]
#[case(ContainerLinkType::Blocks)]
#[case(ContainerLinkType::BlockedBy)]
fn remove_both_directions(#[case] ty: ContainerLinkType) {
    let (conn, kind, a, b) = setup(ContainerKind::Plan);
    create(&conn, kind, a, ContainerLinkType::Blocks, b).unwrap();
    remove(&conn, kind, a, ty, b).unwrap();
    let cnt: i64 = conn
        .query_row("SELECT COUNT(*) FROM container_links", [], |r| r.get(0))
        .unwrap();
    assert_eq!(cnt, 0);
    // 反向视角 + 不存在 → 静默 no-op。
    remove(&conn, kind, b, ty, a).unwrap();
    remove(&conn, kind, a, ty, 999).unwrap();
}

/// links_for_all：只回 blocks 边（from, to），且按 kind 隔离。
#[test]
fn links_for_all_returns_edges() {
    let (conn, kind, a, b) = setup(ContainerKind::Plan);
    create(&conn, kind, a, ContainerLinkType::Blocks, b).unwrap();
    create(&conn, kind, b, ContainerLinkType::BlockedBy, a).unwrap(); // 归一化后与上面同向 → no-op
    assert_eq!(links_for_all(&conn, kind).unwrap(), vec![(a, b)]);
    assert!(
        links_for_all(&conn, ContainerKind::Milestone)
            .unwrap()
            .is_empty()
    );
}
