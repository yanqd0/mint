//! 容器派生状态：issue 6 态 → 容器 5 态的纯函数映射与状态集查询。
//!
//! 供 `sync` 调用完成写回；`derive_status` 为纯函数（无 IO），可独立单测。

use rusqlite::{Connection, params};

use crate::error::Error;
use crate::models::{ContainerStatus, Status};

/// 派生用语义状态（统一 issue 6 态与容器 5 态）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DeriveState {
    Active,
    Done,
    Dropped,
    Open,
}

/// 从 issue 状态转语义状态。
fn derive_from_issue(s: Status) -> DeriveState {
    match s {
        Status::Planned | Status::Dev | Status::Test => DeriveState::Active,
        Status::Done => DeriveState::Done,
        Status::Dropped => DeriveState::Dropped,
        Status::Open => DeriveState::Open,
    }
}

/// 从容器状态转语义状态（running/partial 均视为曾/正活跃）。
fn derive_from_container(s: ContainerStatus) -> DeriveState {
    match s {
        ContainerStatus::Open => DeriveState::Open,
        ContainerStatus::Running | ContainerStatus::Partial => DeriveState::Active,
        ContainerStatus::Dropped => DeriveState::Dropped,
        ContainerStatus::Done => DeriveState::Done,
    }
}

/// 由子项语义状态集合派生容器状态（纯函数）。
/// 优先级：running（任一活跃）> done（全部 done）> dropped（全部 dropped）
///         > partial（恰为 {done,dropped}，无 open 无活跃）> open（全 open/空）。
/// 有任一非 open（含 done/dropped 混 open）→ running（曾/正运行）。
pub(super) fn derive_status(statuses: &[DeriveState]) -> ContainerStatus {
    if statuses.is_empty() {
        return ContainerStatus::Open;
    }
    let mut active = 0;
    let mut done = 0;
    let mut dropped = 0;
    let mut open = 0;
    for s in statuses {
        match s {
            DeriveState::Active => active += 1,
            DeriveState::Done => done += 1,
            DeriveState::Dropped => dropped += 1,
            DeriveState::Open => open += 1,
        }
    }
    if active > 0 {
        return ContainerStatus::Running;
    }
    let total = statuses.len();
    if done == total {
        return ContainerStatus::Done;
    }
    if dropped == total {
        return ContainerStatus::Dropped;
    }
    // 恰为 {done,dropped} 无 open 无活跃 → partial
    if done > 0 && dropped > 0 && open == 0 {
        return ContainerStatus::Partial;
    }
    // 有任一非 open（done 或 dropped，非全 done/全 dropped/纯 done+dropped）→ running（曾运行）
    if done > 0 || dropped > 0 {
        return ContainerStatus::Running;
    }
    ContainerStatus::Open
}

/// 执行查询并转 issue 语义状态。
pub(super) fn issue_statuses_from(
    conn: &Connection,
    sql: &str,
    id: i64,
) -> Result<Vec<DeriveState>, Error> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params![id], |r| r.get::<_, Status>(0))?;
    let statuses = rows.collect::<Result<Vec<_>, _>>().map_err(Error::from)?;
    Ok(statuses.into_iter().map(derive_from_issue).collect())
}

/// 执行查询并转容器语义状态。
pub(super) fn container_statuses_from(
    conn: &Connection,
    sql: &str,
    id: i64,
) -> Result<Vec<DeriveState>, Error> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params![id], |r| r.get::<_, ContainerStatus>(0))?;
    let statuses = rows.collect::<Result<Vec<_>, _>>().map_err(Error::from)?;
    Ok(statuses.into_iter().map(derive_from_container).collect())
}
