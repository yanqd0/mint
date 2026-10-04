//! `--order rank` 排序单测（rstest 参数化）。

use rstest::rstest;

use crate::container::ContainerKind;
use crate::models::ContainerStatus;

use super::*;

/// 造一个最小 Container（仅 id/sort_order 参与排序断言）。
fn mk(id: i64, sort_order: Option<i64>) -> (Container, i64) {
    (
        Container {
            id,
            title: format!("p{id}"),
            version: None,
            body: None,
            milestone_id: None,
            status: ContainerStatus::Open,
            created_at: String::new(),
            updated_at: String::new(),
            sort_order,
        },
        0,
    )
}

/// 排序结果 → id 序列。
fn ids(items: &[(Container, i64)]) -> Vec<i64> {
    items.iter().map(|(c, _)| c.id).collect()
}

/// 无 rank（全 NULL）：保持 id 倒序（与默认 SQL 顺序一致）。
#[test]
fn rank_sort_null_only_falls_back_to_id_desc() {
    let items = vec![mk(1, None), mk(3, None), mk(2, None)];
    let sorted = rank_sorted(items);
    assert_eq!(ids(&sorted), vec![3, 2, 1]);
}

/// 显式 rank 升序在前，未设 rank 的按 id 倒序在后。
#[rstest]
#[case(vec![(1, Some(2)), (2, Some(1)), (3, None), (4, Some(3))], vec![2, 1, 4, 3])]
#[case(vec![(1, None), (2, Some(0))], vec![2, 1])]
#[case(vec![(5, Some(1)), (6, Some(1))], vec![6, 5])]
fn rank_sort_orders_ranked_first(#[case] input: Vec<(i64, Option<i64>)>, #[case] want: Vec<i64>) {
    let items: Vec<(Container, i64)> = input.into_iter().map(|(id, r)| mk(id, r)).collect();
    assert_eq!(ids(&rank_sorted(items)), want);
}

/// `--order rank` 对 milestone list 报错（同 `--milestone only applies to plan list` 先例）。
#[test]
fn order_rank_rejected_for_milestone_list() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    let err = order_containers(
        &conn,
        ContainerKind::Milestone,
        vec![],
        ContainerOrder::Rank,
    )
    .expect_err("milestone list must reject --order rank");
    assert_eq!(err.to_string(), "--order rank only applies to plan list");
}

/// `--order id` 是 no-op（保持 SQL 顺序）。
#[test]
fn order_id_keeps_input_order() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    let items = vec![mk(7, Some(9)), mk(2, None)];
    let sorted = order_containers(&conn, ContainerKind::Plan, items, ContainerOrder::Id).unwrap();
    assert_eq!(ids(&sorted), vec![7, 2]);
}
