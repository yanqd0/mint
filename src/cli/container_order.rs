//! 容器 list 的 `--order` 排序。
//!
//! 默认顺序（`--order id` / 未指定）由 SQL 的 `id DESC` 提供，本模块只在显式指定顺序时重排，
//! 且一律在 `cmd_container_list` 的全部过滤之后、分页之前执行（分页语义不变）。

use std::cmp::Reverse;
use std::collections::HashMap;

use crate::container::{self, ContainerKind};
use crate::error::Error;
use crate::models::Container;

use super::args::ContainerOrder;

/// 按 `--order` 重排过滤后的容器列表。
/// - `Id`：保持 SQL 顺序（id 倒序）。
/// - `Rank`：显式 rank 升序在前，未设 rank（NULL）按 id 倒序在后；仅 plan list 可用。
/// - `Topo`：`blocks` 依赖的拓扑序（阻塞者在前）；成环时确定性回退并打一行 stderr 告警。
pub(crate) fn order_containers(
    conn: &rusqlite::Connection,
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
        ContainerOrder::Topo => {
            let edges = container::links_for_all(conn, kind)?;
            let (sorted, cycle) = topo_sorted(items, &edges);
            if !cycle.is_empty() {
                let ids: Vec<String> = cycle.iter().map(|id| format!("#{id}")).collect();
                eprintln!(
                    "mint: warning: blocks cycle among {} {}; order within cycle is arbitrary",
                    kind.as_str(),
                    ids.join(", ")
                );
            }
            Ok(sorted)
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

/// Kahn 拓扑排序：`(from, to)` 表示 from 阻塞 to → from 先出。
/// 就绪集按 [`sort_key`] 决定的顺序（先 [`rank_sorted`]）取第一个，保证确定性；O(n²) 对容器量级足够。
/// 返回 `(排序结果, 成环节点 id——空 = 无环)`；成环时剩余节点按同一顺序追加（确定性回退，不报错）。
pub(crate) fn topo_sorted(
    items: Vec<(Container, i64)>,
    edges: &[(i64, i64)],
) -> (Vec<(Container, i64)>, Vec<i64>) {
    // 决策序：先按 rank/id 排序，再按该序挑就绪节点。
    let ordered = rank_sorted(items);
    // 只保留两端都在结果集内的边（过滤/分页后的子图）。
    let mut indeg: HashMap<i64, usize> = ordered.iter().map(|(c, _)| (c.id, 0)).collect();
    let edges: Vec<(i64, i64)> = edges
        .iter()
        .copied()
        .filter(|(f, t)| indeg.contains_key(f) && indeg.contains_key(t))
        .collect();
    for (_, t) in &edges {
        if let Some(d) = indeg.get_mut(t) {
            *d += 1;
        }
    }

    let mut remaining: Vec<i64> = ordered.iter().map(|(c, _)| c.id).collect();
    let mut out_ids: Vec<i64> = Vec::with_capacity(remaining.len());
    // 按决策序取第一个入度为 0 的节点。
    while let Some(pos) = remaining.iter().position(|id| indeg[id] == 0) {
        let id = remaining.remove(pos);
        out_ids.push(id);
        for (f, t) in &edges {
            if *f == id
                && let Some(d) = indeg.get_mut(t)
            {
                *d -= 1;
            }
        }
    }
    let cycle = remaining.clone();
    out_ids.extend(remaining);

    let mut by_id: HashMap<i64, (Container, i64)> =
        ordered.into_iter().map(|(c, n)| (c.id, (c, n))).collect();
    let sorted = out_ids
        .into_iter()
        .filter_map(|id| by_id.remove(&id))
        .collect();
    (sorted, cycle)
}

#[cfg(test)]
#[path = "container_order_tests.rs"]
mod tests;
