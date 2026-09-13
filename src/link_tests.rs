//! link.rs 拆分的独立测试模块。

use super::*;
use crate::db;
use rstest::rstest;

fn setup() -> (Connection, i64, i64) {
    let conn = db::open(std::path::Path::new(":memory:")).unwrap();
    conn.execute("INSERT INTO projects (name) VALUES ('p')", [])
        .unwrap();
    conn.execute("INSERT INTO issues (title) VALUES ('a')", [])
        .unwrap();
    conn.execute("INSERT INTO issues (title) VALUES ('b')", [])
        .unwrap();
    let a: i64 = conn
        .query_row("SELECT id FROM issues WHERE title='a'", [], |r| r.get(0))
        .unwrap();
    let b: i64 = conn
        .query_row("SELECT id FROM issues WHERE title='b'", [], |r| r.get(0))
        .unwrap();
    (conn, a, b)
}

/// create 后 links_for 出向/入向正确（类型参数化：rel 与反向 rel）。
#[rstest]
#[case(LinkType::Related, "related", "related")]
#[case(LinkType::Solves, "solves", "solved-by")]
#[case(LinkType::Duplicates, "duplicates", "duplicated-by")]
#[case(LinkType::Blocks, "blocks", "blocked_by")]
fn create_links_for_roundtrip(#[case] ty: LinkType, #[case] rel: &str, #[case] reverse: &str) {
    let (conn, a, b) = setup();
    create(&conn, a, ty, b).unwrap();

    let la = links_for(&conn, a).unwrap();
    assert_eq!(la.len(), 1);
    assert_eq!(la[0].other_id, b);
    assert_eq!(la[0].rel, rel);

    let lb = links_for(&conn, b).unwrap();
    assert_eq!(lb.len(), 1);
    assert_eq!(lb[0].other_id, a);
    assert_eq!(lb[0].rel, reverse);
}

/// 同向重复幂等（类型参数化）：仅 1 行。
#[rstest]
#[case(LinkType::Related)]
#[case(LinkType::Solves)]
#[case(LinkType::Duplicates)]
#[case(LinkType::Blocks)]
fn create_same_direction_idempotent(#[case] ty: LinkType) {
    let (conn, a, b) = setup();
    create(&conn, a, ty, b).unwrap();
    create(&conn, a, ty, b).unwrap();
    let cnt: i64 = conn
        .query_row("SELECT COUNT(*) FROM issue_links", [], |r| r.get(0))
        .unwrap();
    assert_eq!(cnt, 1);
}

/// related 反向对称 no-op：A related B 再 B related A → 仍 1 行。
#[test]
fn create_reverse_related_idempotent() {
    let (conn, a, b) = setup();
    create(&conn, a, LinkType::Related, b).unwrap();
    create(&conn, b, LinkType::Related, a).unwrap(); // 归一化后同主键
    let cnt: i64 = conn
        .query_row("SELECT COUNT(*) FROM issue_links", [], |r| r.get(0))
        .unwrap();
    assert_eq!(cnt, 1);
}

/// 反向冲突报错（solves/duplicates/blocks 有向类型反向互斥）。
#[rstest]
#[case(LinkType::Solves)]
#[case(LinkType::Duplicates)]
#[case(LinkType::Blocks)]
fn create_reverse_directional_conflict(#[case] ty: LinkType) {
    let (conn, a, b) = setup();
    create(&conn, a, ty, b).unwrap();
    let err = create(&conn, b, ty, a).unwrap_err();
    assert!(err.to_string().contains("already linked"), "err: {err}");
}

/// 自环报错。
#[test]
fn create_self_link_error() {
    let (conn, a, _) = setup();
    let err = create(&conn, a, LinkType::Related, a).unwrap_err();
    assert!(err.to_string().contains("to itself"), "err: {err}");
}

/// 端缺失报错。
#[test]
fn create_missing_issue_error() {
    let (conn, a, _) = setup();
    let err = create(&conn, a, LinkType::Related, 999).unwrap_err();
    assert!(
        err.to_string().contains("issue #999 not found"),
        "err: {err}"
    );
}

/// remove 对称：存 A related B，remove(B, related, A) 能删。
#[test]
fn remove_reverse_fallback() {
    let (conn, a, b) = setup();
    create(&conn, a, LinkType::Related, b).unwrap();
    remove(&conn, b, LinkType::Related, a).unwrap();
    let cnt: i64 = conn
        .query_row("SELECT COUNT(*) FROM issue_links", [], |r| r.get(0))
        .unwrap();
    assert_eq!(cnt, 0);
    // 再 remove no-op
    remove(&conn, b, LinkType::Related, a).unwrap();
}

/// blocked_by 归一化后用户表述 A blocked_by B 能删（归一化到 (B, blocks, A)）。
#[test]
fn remove_blocked_by_user_view_deletes() {
    let (conn, a, b) = setup();
    create(&conn, a, LinkType::BlockedBy, b).unwrap();
    remove(&conn, a, LinkType::BlockedBy, b).unwrap();
    let cnt: i64 = conn
        .query_row("SELECT COUNT(*) FROM issue_links", [], |r| r.get(0))
        .unwrap();
    assert_eq!(cnt, 0, "blocked_by 用户表述应删到 blocks 行");
}

/// blocked_by 反向（B blocks A 或 B blocked_by A）表述也能删。
#[test]
fn remove_blocked_by_reverse_view_deletes() {
    let (conn, a, b) = setup();
    create(&conn, a, LinkType::BlockedBy, b).unwrap();
    // B blocks A 出向表述
    remove(&conn, b, LinkType::Blocks, a).unwrap();
    let cnt: i64 = conn
        .query_row("SELECT COUNT(*) FROM issue_links", [], |r| r.get(0))
        .unwrap();
    assert_eq!(cnt, 0, "blocks 出向表述应删到");
    // 重建后 B blocked_by A（反向用户表述）也删
    create(&conn, a, LinkType::BlockedBy, b).unwrap();
    remove(&conn, b, LinkType::BlockedBy, a).unwrap();
    let cnt2: i64 = conn
        .query_row("SELECT COUNT(*) FROM issue_links", [], |r| r.get(0))
        .unwrap();
    assert_eq!(cnt2, 0, "blocked_by 反向表述应删到");
}

/// blocked_by 归一化为 blocks（方向互换）：A blocked_by B ≡ B blocks A（幂等）。
#[test]
fn blocked_by_normalizes_to_blocks_idempotent() {
    let (conn, a, b) = setup();
    create(&conn, a, LinkType::BlockedBy, b).unwrap();
    // 库中存储：A blocked_by B 归一化为 (B, blocks, A)
    let cnt: i64 = conn
        .query_row("SELECT COUNT(*) FROM issue_links", [], |r| r.get(0))
        .unwrap();
    assert_eq!(cnt, 1);
    // A 视角（入向 reverse）：blocked_by B
    let la = links_for(&conn, a).unwrap();
    assert_eq!(la[0].rel, "blocked_by");
    assert_eq!(la[0].other_id, b);
    // B 视角（出向）：blocks A
    let lb = links_for(&conn, b).unwrap();
    assert_eq!(lb[0].rel, "blocks");
    assert_eq!(lb[0].other_id, a);
    // B blocks A 再调 → 幂等（库中同向已存在）
    create(&conn, b, LinkType::Blocks, a).unwrap();
    let cnt2: i64 = conn
        .query_row("SELECT COUNT(*) FROM issue_links", [], |r| r.get(0))
        .unwrap();
    assert_eq!(cnt2, 1);
}

/// links_for_many 批量结果与逐 issue links_for 的 (other_id, rel) 序列一致。
#[test]
fn links_for_many_matches_links_for() {
    let (conn, a, b) = setup();
    create(&conn, a, LinkType::Solves, b).unwrap();
    create(&conn, a, LinkType::Related, b).unwrap();
    let map = links_for_many(&conn).unwrap();
    for issue_id in [a, b] {
        let batch: Vec<(i64, String)> = map
            .get(&issue_id)
            .map(|ls| ls.iter().map(|l| (l.other_id, l.rel.clone())).collect())
            .unwrap_or_default();
        let single: Vec<(i64, String)> = links_for(&conn, issue_id)
            .unwrap()
            .into_iter()
            .map(|l| (l.other_id, l.rel))
            .collect();
        assert_eq!(batch, single, "issue {issue_id} 批量应等于逐条");
    }
}

/// 多类型混合聚合 + 排序（出向在前、入向在后）。
#[test]
fn links_for_multi_types_ordered() {
    let (conn, a, b) = setup();
    conn.execute("INSERT INTO issues (title) VALUES ('c')", [])
        .unwrap();
    let c: i64 = conn
        .query_row("SELECT id FROM issues WHERE title='c'", [], |r| r.get(0))
        .unwrap();
    create(&conn, a, LinkType::Solves, b).unwrap();
    create(&conn, a, LinkType::Related, c).unwrap();
    create(&conn, b, LinkType::Duplicates, a).unwrap(); // 入向

    let la = links_for(&conn, a).unwrap();
    // 出向先：solves b、related c；入向后：duplicated-by b
    let rels: Vec<&str> = la.iter().map(|l| l.rel.as_str()).collect();
    assert_eq!(rels, vec!["solves", "related", "duplicated-by"]);
}
