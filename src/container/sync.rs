//! 容器派生状态写回：issue 状态变更后的级联同步（issue → plan → milestone），
//! 以及容器状态的手动终态设置。

use rusqlite::{Connection, params};

use crate::db;
use crate::error::Error;
use crate::models::ContainerStatus;

use super::derive::{container_statuses_from, derive_status, issue_statuses_from};
use super::{ContainerKind, get};

/// 写后级联同步：某 issue 状态/归属变化后，重算其所属 plan 与 milestone 的状态。
/// 事务由调用方保证（transition 等写路径在事务内调用）。
pub fn sync_container_status(conn: &Connection, issue_id: i64) -> Result<(), Error> {
    // plan 同步
    let plan_ids: Vec<i64> = conn
        .prepare(db::PLAN_IDS_FOR_ISSUE)?
        .query_map(params![issue_id], |r| r.get(0))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(Error::from)?;
    for pid in plan_ids {
        sync_plan(conn, pid)?;
    }
    // milestone 直接挂的同步
    let r_ids: Vec<i64> = conn
        .prepare(db::MILESTONE_IDS_FOR_ISSUE)?
        .query_map(params![issue_id], |r| r.get(0))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(Error::from)?;
    for rid in r_ids {
        sync_milestone(conn, rid)?;
    }
    Ok(())
}

/// 重算某 plan 状态并写回；随后同步其所属 milestone。
pub(super) fn sync_plan(conn: &Connection, plan_id: i64) -> Result<(), Error> {
    let statuses = issue_statuses_from(conn, db::PLAN_ISSUE_STATUSES, plan_id)?;
    // #446：空 plan 的手动 dropped（`plan drop` 仅允许空 plan）不被派生覆盖——空集合派生为
    // open，否则任何同步入口（如 `plan set --milestone`）都会把显式废弃的 plan 复活。
    // 非空 plan 的 dropped 是派生结果（其下 issue 全 dropped），仍随后续状态变化重算。
    let manual_drop = statuses.is_empty()
        && get(conn, ContainerKind::Plan, plan_id)?.map(|c| c.status)
            == Some(ContainerStatus::Dropped);
    if !manual_drop {
        let st = derive_status(&statuses);
        conn.execute(db::PLAN_UPDATE_STATUS, params![st, plan_id])?;
    }
    // plan 变更 → 检查 milestone（手动 dropped 时 plan 状态未变，但归属可能已变，仍需同步）
    let milestone_ids: Vec<i64> = conn
        .prepare(db::MILESTONE_IDS_FOR_PLAN)?
        .query_map(params![plan_id], |r| r.get(0))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(Error::from)?;
    for rid in milestone_ids {
        sync_milestone(conn, rid)?;
    }
    Ok(())
}

/// 重算某 milestone 状态（plan 状态 + 直接挂 issue 状态合并）并写回。
pub(super) fn sync_milestone(conn: &Connection, milestone_id: i64) -> Result<(), Error> {
    // 手动终态（done=已发布 / dropped=已取消，由 `milestone set --status` 单独操作产生）不被派生覆盖。
    let current = get(conn, ContainerKind::Milestone, milestone_id)?
        .map(|c| c.status)
        .unwrap_or(ContainerStatus::Open);
    if matches!(current, ContainerStatus::Done | ContainerStatus::Dropped) {
        return Ok(());
    }
    let mut statuses = container_statuses_from(conn, db::MILESTONE_PLAN_STATUSES, milestone_id)?;
    statuses.extend(issue_statuses_from(
        conn,
        db::MILESTONE_DIRECT_ISSUE_STATUSES,
        milestone_id,
    )?);
    let st = derive_status(&statuses);
    // milestone 是版本桶：不随子项全部 done/dropped 自动 close/drop（需显式发布/取消），
    // 派生结果 done/dropped → running（版本进行中待发布）；其余（open/running/partial）保留自动派生。
    let st = match st {
        ContainerStatus::Done | ContainerStatus::Dropped => ContainerStatus::Running,
        other => other,
    };
    conn.execute(db::MILESTONE_UPDATE_STATUS, params![st, milestone_id])?;
    Ok(())
}

/// 手动设置 plan 状态（丢弃空 plan → dropped）。
pub fn set_plan_status(
    conn: &Connection,
    plan_id: i64,
    status: ContainerStatus,
) -> Result<(), Error> {
    let affected = conn.execute(db::PLAN_UPDATE_STATUS, params![status, plan_id])?;
    if affected == 0 {
        return Err(Error::Other(format!("plan #{plan_id} not found")));
    }
    Ok(())
}

/// 手动设置 milestone 状态（发布 → done；取消 → dropped）。done/dropped 为终态，派生不覆盖。
pub fn set_milestone_status(
    conn: &Connection,
    milestone_id: i64,
    status: ContainerStatus,
) -> Result<(), Error> {
    let affected = conn.execute(db::MILESTONE_UPDATE_STATUS, params![status, milestone_id])?;
    if affected == 0 {
        return Err(Error::Other(format!("milestone #{milestone_id} not found")));
    }
    Ok(())
}
