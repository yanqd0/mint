//! 容器 list 的 `--order` 排序。
//!
//! 默认顺序（`--order id` / 未指定）由 SQL 的 `id DESC` 提供，本模块只在显式指定顺序时重排，
//! 且一律在 `cmd_container_list` 的全部过滤之后、分页之前执行（分页语义不变）。

use std::cmp::Reverse;

use crate::container::ContainerKind;
use crate::error::Error;
use crate::models::Container;

use super::args::ContainerOrder;

/// 按 `--order` 重排过滤后的容器列表。
/// - `Id`：保持 SQL 顺序（id 倒序）。
/// - `Rank`：显式 rank 升序在前，未设 rank（NULL）按 id 倒序在后；仅 plan list 可用。
pub(crate) fn order_containers(
    _conn: &rusqlite::Connection,
    kind: ContainerKind,
    items: Vec<(Container, i64)>,
    order: ContainerOrder,
) -> Result<Vec<(Container, i64)>, Error> {
    match order {
        ContainerOrder::Id => Ok(items),
        ContainerOrder::Rank => {
            if kind != ContainerKind::Plan {
                return Err(Error::Other(
                    "--order rank only applies to plan list".to_string(),
                ));
            }
            Ok(rank_sorted(items))
        }
    }
}

/// rank 排序键：`(未设 rank, rank, id DESC)`——显式 rank 升序在前，NULL 末位回落 id 倒序。
/// 供 `--order rank` 与拓扑排序的并列决胜共用。
pub(crate) fn sort_key(c: &Container) -> (bool, i64, Reverse<i64>) {
    (
        c.sort_order.is_none(),
        c.sort_order.unwrap_or_default(),
        Reverse(c.id),
    )
}

/// 按 [`sort_key`] 稳定排序。
pub(crate) fn rank_sorted(items: Vec<(Container, i64)>) -> Vec<(Container, i64)> {
    let mut items = items;
    items.sort_by_key(|(c, _)| sort_key(c));
    items
}

#[cfg(test)]
#[path = "container_order_tests.rs"]
mod tests;
