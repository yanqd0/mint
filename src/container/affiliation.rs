//! 容器归属：plan/milestone 与 issue 的挂载、解绑与迁移（含事务内派生状态重算）。

use rusqlite::{Connection, OptionalExtension, params};

use crate::db;
use crate::error::Error;
use crate::models::IssueSummary;

use super::sync::{is_manual_dropped, sync_container_status, sync_milestone, sync_plan};
use super::{ContainerKind, get};

/// 查询容器下的 issue 摘要。
pub fn issues_for(
    conn: &Connection,
    kind: ContainerKind,
    id: i64,
) -> Result<Vec<IssueSummary>, Error> {
    let sql = match kind {
        ContainerKind::Milestone => db::MILESTONE_ISSUES_FOR,
        ContainerKind::Plan => db::PLAN_ISSUES_FOR,
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params![id], |r| {
        Ok(IssueSummary {
            id: r.get(0)?,
            title: r.get(1)?,
            kind: r.get(2)?,
            status: r.get(3)?,
            project: r.get(4)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Error::from)
}

/// 当前 issue 的容器归属（plan + 直属 milestone），供归属变更前记录源端。
fn current_affiliations(conn: &Connection, issue_id: i64) -> Result<(Vec<i64>, Vec<i64>), Error> {
    let plans: Vec<i64> = conn
        .prepare(db::PLAN_IDS_FOR_ISSUE)?
        .query_map(params![issue_id], |r| r.get(0))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(Error::from)?;
    let milestones: Vec<i64> = conn
        .prepare(db::MILESTONE_IDS_FOR_ISSUE)?
        .query_map(params![issue_id], |r| r.get(0))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(Error::from)?;
    Ok((plans, milestones))
}

/// 归属变更 + 容器状态重算（含源端），同一事务内原子；拒绝嵌套事务（同 delete_txn）。
/// `write` 执行归属变更；之后重算该 issue 当前容器（sync_container_status）+ 显式重算
/// 源容器（旧 plan/旧直属 milestone，写入后已不在 PLAN_IDS/MILESTONE_IDS 中）。
fn reassign_container(
    conn: &Connection,
    issue_id: i64,
    old_plans: &[i64],
    old_milestones: &[i64],
    write: impl FnOnce(&Connection) -> Result<(), Error>,
) -> Result<(), Error> {
    if !conn.is_autocommit() {
        return Err(Error::Other(
            "container reassign must not run inside another transaction".to_string(),
        ));
    }
    conn.execute_batch("BEGIN IMMEDIATE")?;
    let result = (|| {
        // 唯一 running 守卫的 before 快照（#104）：全部派生同步完成后比对，净计数增加即整体回滚。
        let before = super::running_milestones(conn)?;
        write(conn)?;
        sync_container_status(conn, issue_id)?;
        for &p in old_plans {
            sync_plan(conn, p)?;
        }
        for &m in old_milestones {
            sync_milestone(conn, m)?;
        }
        super::ensure_running_not_increased(conn, &before)?;
        Ok(())
    })();
    match result {
        Ok(()) => {
            conn.execute_batch("COMMIT")?;
            Ok(())
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

/// milestone 直接挂 issue（仅接受无 plan 的 issue，且至多一个直挂 milestone）。幂等。
pub fn link_direct(conn: &Connection, milestone_id: i64, issue_id: i64) -> Result<(), Error> {
    if get(conn, ContainerKind::Milestone, milestone_id)?.is_none() {
        return Err(Error::Other(format!("milestone #{milestone_id} not found")));
    }
    let plan_id: Option<Option<i64>> = conn
        .query_row(db::ISSUE_SELECT_PLAN_ID, params![issue_id], |r| r.get(0))
        .optional()
        .map_err(Error::from)?;
    match plan_id {
        None => return Err(Error::Other(format!("issue #{issue_id} not found"))),
        Some(Some(_)) => {
            return Err(Error::Other(format!(
                "issue #{issue_id} already belongs to a plan; unassign it first"
            )));
        }
        Some(None) => {}
    }
    let (old_plans, old_milestones) = current_affiliations(conn, issue_id)?;
    // 直挂至多一个 milestone（#496）：多条直挂让「有效 milestone」标量子查询取值不定，
    // 使 `milestone show`/TUI 与 `issue get milestone`/`list --milestone` 结果矛盾。
    if let Some(other) = old_milestones.iter().find(|m| **m != milestone_id) {
        return Err(Error::Other(format!(
            "issue #{issue_id} already belongs to milestone #{other}; detach it first"
        )));
    }
    reassign_container(conn, issue_id, &old_plans, &old_milestones, |conn| {
        conn.execute(db::MILESTONE_ATTACH, params![milestone_id, issue_id])?;
        Ok(())
    })
}

/// 解除 milestone 直接挂的 issue。milestone/issue 不存在报错（与 attach 校验对齐）。
pub fn unlink_direct(conn: &Connection, milestone_id: i64, issue_id: i64) -> Result<(), Error> {
    if get(conn, ContainerKind::Milestone, milestone_id)?.is_none() {
        return Err(Error::Other(format!("milestone #{milestone_id} not found")));
    }
    ensure_issue_exists(conn, issue_id)?;
    let (old_plans, old_milestones) = current_affiliations(conn, issue_id)?;
    reassign_container(conn, issue_id, &old_plans, &old_milestones, |conn| {
        conn.execute(db::MILESTONE_DETACH, params![milestone_id, issue_id])?;
        Ok(())
    })
}

/// 把 issue 挂到 plan 下（plan_id 外键）。plan 不存在报错；手动 dropped 的 plan 拒绝挂载（#497）。
pub fn set_issue_plan(conn: &Connection, issue_id: i64, plan_id: i64) -> Result<(), Error> {
    if get(conn, ContainerKind::Plan, plan_id)?.is_none() {
        return Err(Error::Other(format!("plan #{plan_id} not found")));
    }
    if is_manual_dropped(conn, plan_id)? {
        return Err(Error::Other(format!(
            "plan #{plan_id} is dropped; cannot attach issues"
        )));
    }
    ensure_issue_exists(conn, issue_id)?;
    let (old_plans, old_milestones) = current_affiliations(conn, issue_id)?;
    reassign_container(conn, issue_id, &old_plans, &old_milestones, |conn| {
        // 若该 issue 已直接挂 milestone，需先解除（二选一）
        conn.execute(db::MILESTONE_DIRECT_DELETE_BY_ISSUE, params![issue_id])?;
        conn.execute(db::ISSUE_SET_PLAN, params![plan_id, issue_id])?;
        Ok(())
    })
}

/// 解除 issue 的 plan 归属（plan_id 置 NULL）。issue 不存在报错（#341 与 attach 对齐）。
pub fn unset_issue_plan(conn: &Connection, issue_id: i64) -> Result<(), Error> {
    ensure_issue_exists(conn, issue_id)?;
    let (old_plans, old_milestones) = current_affiliations(conn, issue_id)?;
    reassign_container(conn, issue_id, &old_plans, &old_milestones, |conn| {
        conn.execute(db::ISSUE_UNSET_PLAN, params![issue_id])?;
        Ok(())
    })
}

/// 校验 issue 存在；不存在报错（attach/detach 对称校验，#341）。
fn ensure_issue_exists(conn: &Connection, issue_id: i64) -> Result<(), Error> {
    let exists: Option<i64> = conn
        .query_row(db::ISSUE_EXISTS, params![issue_id], |r| r.get(0))
        .optional()
        .map_err(Error::from)?;
    if exists.is_none() {
        return Err(Error::Other(format!("issue #{issue_id} not found")));
    }
    Ok(())
}
