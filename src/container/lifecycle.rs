//! 容器生命周期：更新、跨桶移动与删除（多语句关联操作在同一事务内原子提交）。

use rusqlite::{Connection, OptionalExtension, params};

use crate::db;
use crate::error::Error;

use super::sync::{sync_milestone, sync_plan};
use super::{ContainerKind, get};

/// 更新 plan 的 title/body（COALESCE 保留未提供字段）。
/// 不涉及派生状态同步（title/body 变更不影响 plan 状态）。
pub fn update_plan(
    conn: &Connection,
    id: i64,
    title: Option<&str>,
    body: Option<&str>,
) -> Result<(), Error> {
    let affected = conn.execute(db::PLAN_UPDATE, params![id, title, body])?;
    if affected == 0 {
        return Err(Error::Other(format!("plan #{id} not found")));
    }
    Ok(())
}

/// 更新 milestone 的 title/version/body（COALESCE 保留未提供字段）。
/// 不涉及派生状态同步（title/version/body 变更不影响 milestone 状态）。
pub fn update_milestone(
    conn: &Connection,
    id: i64,
    title: Option<&str>,
    version: Option<&str>,
    body: Option<&str>,
) -> Result<(), Error> {
    let affected = conn.execute(db::MILESTONE_UPDATE, params![id, title, version, body])?;
    if affected == 0 {
        return Err(Error::Other(format!("milestone #{id} not found")));
    }
    Ok(())
}

/// 执行删除（多语句 SQL 的关联操作）+ `after` 同步，同一 `BEGIN IMMEDIATE...COMMIT` 事务内原子提交。
/// 任一失败整体回滚，使"删除 + 派生状态同步"不可分割；拒绝在既有事务内调用（避免误回滚外层事务）。
/// SQL 仅单参数 `?1`；id 为自增数字，替换安全（无注入面）。
pub(crate) fn delete_txn(
    conn: &Connection,
    sql: &str,
    id: i64,
    after: impl FnOnce(&Connection) -> Result<(), Error>,
) -> Result<(), Error> {
    if !conn.is_autocommit() {
        return Err(Error::Other(
            "delete must not run inside another transaction".to_string(),
        ));
    }
    conn.execute_batch("BEGIN IMMEDIATE")?;
    let result = (|| {
        // 逐语句参数化执行：execute_batch 不支持绑定参数，原 `sql.replace("?1", id)` 有
        // 误替换（字面量含 ?1 / ?10）风险；delete SQL 均仅单参数 `?1`、注释/语句间无分号，
        // 按 ';' 拆分 + params![id] 绑定安全。
        for stmt in sql.split(';').map(str::trim).filter(|s| !s.is_empty()) {
            conn.execute(stmt, params![id])?;
        }
        after(conn)?;
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

/// 删除 plan：关联操作（解绑其下全部 issue 的 plan_id + 删 plan）与派生状态同步在同一事务。
pub fn delete_plan(conn: &Connection, id: i64) -> Result<(), Error> {
    let c = get(conn, ContainerKind::Plan, id)?
        .ok_or_else(|| Error::Other(format!("plan #{id} not found")))?;
    let milestone_id = c.milestone_id;
    delete_txn(conn, db::PLAN_DELETE, id, |conn| {
        if let Some(rid) = milestone_id {
            sync_milestone(conn, rid)?;
        }
        Ok(())
    })
}

/// 移动 plan 到另一 milestone：更新 milestone_id，将其下 planned issue 重置回 open
/// （跨桶排期作废），并重算 plan 状态与两侧 milestone 派生状态，同一事务内原子。
/// 旧侧不再含该 plan（派生回落），新侧纳入该 plan（派生推进）。拒绝嵌套事务（同 delete_txn）。
/// 返回被重置（planned → open）的 issue 数。
pub fn move_plan(conn: &Connection, id: i64, new_milestone_id: i64) -> Result<usize, Error> {
    let plan = get(conn, ContainerKind::Plan, id)?
        .ok_or_else(|| Error::Other(format!("plan #{id} not found")))?;
    // 校验新 milestone 存在（FK 下不存在会报原始约束错；显式校验给友好报错，对齐 link_direct）。
    if get(conn, ContainerKind::Milestone, new_milestone_id)?.is_none() {
        return Err(Error::Other(format!(
            "milestone #{new_milestone_id} not found"
        )));
    }
    let old = plan.milestone_id;
    if old == Some(new_milestone_id) {
        return Ok(0); // 同 milestone 迁移：no-op（不重置排期）
    }
    if !conn.is_autocommit() {
        return Err(Error::Other(
            "plan move must not run inside another transaction".to_string(),
        ));
    }
    conn.execute_batch("BEGIN IMMEDIATE")?;
    let result = (|| {
        conn.execute(db::PLAN_SET_MILESTONE, params![new_milestone_id, id])?;
        // 跨桶移动 = 排期上下文变更：planned（已排期未开始）作废回 open，由新归属重新排期；
        // dev/test/done/dropped 不动（进行中/已完成与版本桶归属无关）。
        let reset = conn.execute(db::PLAN_RESET_PLANNED, params![id])?;
        sync_plan(conn, id)?; // plan 状态重算（其下 issue 已变），并同步新侧 milestone
        if let Some(rid) = old {
            sync_milestone(conn, rid)?; // 旧侧重算（回落）
        }
        Ok(reset)
    })();
    match result {
        Ok(reset) => {
            conn.execute_batch("COMMIT")?;
            Ok(reset)
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

/// 删除 milestone：关联操作（清直接挂载 + 解绑其下 plan 的 milestone_id + 删 milestone）在同一事务。
pub fn delete_milestone(conn: &Connection, id: i64) -> Result<(), Error> {
    if get(conn, ContainerKind::Milestone, id)?.is_none() {
        return Err(Error::Other(format!("milestone #{id} not found")));
    }
    delete_txn(conn, db::MILESTONE_DELETE, id, |_| Ok(()))
}

/// 物理删除 issue：关联操作（清 label/links/milestone 挂载 + 删行）与所属容器派生状态同步在同一事务。
/// 所属容器在删除前记录（删除后无法查询）。
pub fn delete_issue(conn: &Connection, id: i64) -> Result<(), Error> {
    let exists: Option<i64> = conn
        .query_row(db::ISSUE_EXISTS, params![id], |r| r.get(0))
        .optional()
        .map_err(Error::from)?;
    if exists.is_none() {
        return Err(Error::Other(format!("issue #{id} not found")));
    }
    let plan_ids: Vec<i64> = conn
        .prepare(db::PLAN_IDS_FOR_ISSUE)?
        .query_map(params![id], |r| r.get(0))?
        .collect::<Result<_, _>>()
        .map_err(Error::from)?;
    let milestone_ids: Vec<i64> = conn
        .prepare(db::MILESTONE_IDS_FOR_ISSUE)?
        .query_map(params![id], |r| r.get(0))?
        .collect::<Result<_, _>>()
        .map_err(Error::from)?;
    delete_txn(conn, db::ISSUE_DELETE, id, |conn| {
        for pid in &plan_ids {
            sync_plan(conn, *pid)?;
        }
        for rid in &milestone_ids {
            sync_milestone(conn, *rid)?;
        }
        Ok(())
    })
}
