//! 容器（milestone/plan）共享模块：聚合 issue/plan。
//!
//! 层级：milestone → plan（milestone_id）→ issue（plan_id）；milestone 可直接挂无 plan 的 issue
//! （milestone_direct_issues，二选一约束）。容器状态 5 态派生（写后同步，CLI 只读）。

use rusqlite::{Connection, OptionalExtension, params};

use crate::db;
use crate::error::Error;
use crate::models::{Container, ContainerStatus, IssueSummary, Status};

/// 容器类型：milestone / plan。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerKind {
    Milestone,
    Plan,
}

impl ContainerKind {
    fn insert_sql(self) -> &'static str {
        match self {
            ContainerKind::Milestone => db::MILESTONE_INSERT,
            ContainerKind::Plan => db::PLAN_INSERT,
        }
    }

    fn list_sql(self) -> &'static str {
        match self {
            ContainerKind::Milestone => db::MILESTONE_LIST,
            ContainerKind::Plan => db::PLAN_LIST,
        }
    }

    fn select_sql(self) -> &'static str {
        match self {
            ContainerKind::Milestone => db::MILESTONE_SELECT,
            ContainerKind::Plan => db::PLAN_SELECT,
        }
    }
}

/// 派生用语义状态（统一 issue 6 态与容器 5 态）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeriveState {
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
fn derive_status(statuses: &[DeriveState]) -> ContainerStatus {
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

/// 新建容器，返回 id。milestone 必填 version；plan 可带 milestone_id。
pub fn create(
    conn: &Connection,
    kind: ContainerKind,
    title: &str,
    version: Option<&str>,
    body: Option<&str>,
    milestone_id: Option<i64>,
) -> Result<i64, Error> {
    match kind {
        ContainerKind::Milestone => {
            let v = version.filter(|s| !s.trim().is_empty());
            if v.is_none() {
                return Err(Error::Other("milestone requires --version".to_string()));
            }
            conn.execute(kind.insert_sql(), params![title, v, body])?;
        }
        ContainerKind::Plan => {
            conn.execute(kind.insert_sql(), params![title, body, milestone_id])?;
        }
    }
    Ok(conn.last_insert_rowid())
}

/// 列出全部容器（含子项计数），(容器, 计数)。
/// `status: Some(..)` 时按容器状态过滤（显式 status 放开 done 排除，对齐 issue_list）。
pub fn list(
    conn: &Connection,
    kind: ContainerKind,
    all: bool,
    status: Option<ContainerStatus>,
) -> Result<Vec<(Container, i64)>, Error> {
    let all_flag: i64 = if all { 1 } else { 0 };
    let mut stmt = conn.prepare(kind.list_sql())?;
    let rows = stmt.query_map(params![all_flag, status], |r| {
        let container = Container {
            id: r.get(0)?,
            title: r.get(1)?,
            version: r.get(2)?,
            body: r.get(3)?,
            milestone_id: r.get(4)?,
            status: r.get(5)?,
            created_at: r.get(6)?,
            updated_at: r.get(7)?,
        };
        let count: i64 = r.get(8)?;
        Ok((container, count))
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Error::from)
}

/// 查询单条容器（不存在返回 None）。
pub fn get(conn: &Connection, kind: ContainerKind, id: i64) -> Result<Option<Container>, Error> {
    conn.query_row(kind.select_sql(), params![id], |r| {
        Ok(Container {
            id: r.get(0)?,
            title: r.get(1)?,
            version: r.get(2)?,
            body: r.get(3)?,
            milestone_id: r.get(4)?,
            status: r.get(5)?,
            created_at: r.get(6)?,
            updated_at: r.get(7)?,
        })
    })
    .optional()
    .map_err(Error::from)
}

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
        write(conn)?;
        sync_container_status(conn, issue_id)?;
        for &p in old_plans {
            sync_plan(conn, p)?;
        }
        for &m in old_milestones {
            sync_milestone(conn, m)?;
        }
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

/// milestone 直接挂 issue（仅接受无 plan 的 issue）。幂等。
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

/// 把 issue 挂到 plan 下（plan_id 外键）。plan 不存在报错。
pub fn set_issue_plan(conn: &Connection, issue_id: i64, plan_id: i64) -> Result<(), Error> {
    if get(conn, ContainerKind::Plan, plan_id)?.is_none() {
        return Err(Error::Other(format!("plan #{plan_id} not found")));
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
fn sync_plan(conn: &Connection, plan_id: i64) -> Result<(), Error> {
    let statuses = issue_statuses_from(conn, db::PLAN_ISSUE_STATUSES, plan_id)?;
    let st = derive_status(&statuses);
    conn.execute(db::PLAN_UPDATE_STATUS, params![st, plan_id])?;
    // plan 变更 → 检查 milestone
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
fn sync_milestone(conn: &Connection, milestone_id: i64) -> Result<(), Error> {
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

/// 执行查询并转 issue 语义状态。
fn issue_statuses_from(conn: &Connection, sql: &str, id: i64) -> Result<Vec<DeriveState>, Error> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params![id], |r| r.get::<_, Status>(0))?;
    let statuses = rows.collect::<Result<Vec<_>, _>>().map_err(Error::from)?;
    Ok(statuses.into_iter().map(derive_from_issue).collect())
}

/// 执行查询并转容器语义状态。
fn container_statuses_from(
    conn: &Connection,
    sql: &str,
    id: i64,
) -> Result<Vec<DeriveState>, Error> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params![id], |r| r.get::<_, ContainerStatus>(0))?;
    let statuses = rows.collect::<Result<Vec<_>, _>>().map_err(Error::from)?;
    Ok(statuses.into_iter().map(derive_from_container).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use rstest::rstest;

    fn setup() -> (Connection, i64) {
        let conn = db::open(std::path::Path::new(":memory:")).unwrap();
        conn.execute("INSERT INTO projects (name) VALUES ('p')", [])
            .unwrap();
        conn.execute("INSERT INTO issues (title) VALUES ('a')", [])
            .unwrap();
        let iid: i64 = conn
            .query_row("SELECT id FROM issues", [], |r| r.get(0))
            .unwrap();
        (conn, iid)
    }

    fn set_status(conn: &Connection, id: i64, st: &str) {
        conn.execute(
            "UPDATE issues SET status = ?1 WHERE id = ?2",
            params![st, id],
        )
        .unwrap();
    }

    /// derive_status 全分支参数化：子项状态组合 → 容器状态。
    #[rstest]
    #[case(&[], ContainerStatus::Open)]
    #[case(&[DeriveState::Open, DeriveState::Open], ContainerStatus::Open)]
    #[case(&[DeriveState::Active], ContainerStatus::Running)]
    #[case(&[DeriveState::Active, DeriveState::Done], ContainerStatus::Running)]
    #[case(&[DeriveState::Done, DeriveState::Done], ContainerStatus::Done)]
    #[case(&[DeriveState::Dropped, DeriveState::Dropped], ContainerStatus::Dropped)]
    #[case(&[DeriveState::Done, DeriveState::Dropped], ContainerStatus::Partial)]
    #[case(&[DeriveState::Open, DeriveState::Done], ContainerStatus::Running)]
    #[case(&[DeriveState::Open, DeriveState::Dropped], ContainerStatus::Running)]
    #[case(
        &[DeriveState::Open, DeriveState::Done, DeriveState::Dropped],
        ContainerStatus::Running
    )]
    fn derive_status_cases(#[case] states: &[DeriveState], #[case] expected: ContainerStatus) {
        assert_eq!(derive_status(states), expected);
    }

    /// milestone create 必填 version。
    #[test]
    fn create_requires_version() {
        let (conn, _) = setup();
        let err = create(&conn, ContainerKind::Milestone, "r", None, None, None).unwrap_err();
        assert!(err.to_string().contains("--version"));
        let id = create(
            &conn,
            ContainerKind::Milestone,
            "r",
            Some("0.1.0"),
            None,
            None,
        )
        .unwrap();
        assert!(id > 0);
    }

    /// milestone 直接挂 issue（无 plan）+ 派生状态同步。
    #[test]
    fn milestone_direct_issue_derives_status() {
        let (conn, iid) = setup();
        let rid = create(
            &conn,
            ContainerKind::Milestone,
            "r",
            Some("0.1.0"),
            None,
            None,
        )
        .unwrap();
        link_direct(&conn, rid, iid).unwrap();
        assert_eq!(
            get(&conn, ContainerKind::Milestone, rid)
                .unwrap()
                .unwrap()
                .status,
            ContainerStatus::Open
        );

        set_status(&conn, iid, "done");
        sync_container_status(&conn, iid).unwrap();
        // milestone 不随子项全部完成自动 done：派生 done → running（版本进行中待发布）。
        assert_eq!(
            get(&conn, ContainerKind::Milestone, rid)
                .unwrap()
                .unwrap()
                .status,
            ContainerStatus::Running
        );
    }

    /// 手动 done（发布）不被后续派生覆盖。
    #[test]
    fn milestone_manual_done_not_overwritten_by_sync() {
        let (conn, iid) = setup();
        let rid = create(
            &conn,
            ContainerKind::Milestone,
            "r",
            Some("0.1.0"),
            None,
            None,
        )
        .unwrap();
        link_direct(&conn, rid, iid).unwrap();
        set_milestone_status(&conn, rid, ContainerStatus::Done).unwrap();
        // 子项状态变化触发 sync → milestone 保持手动 done。
        set_status(&conn, iid, "dev");
        sync_container_status(&conn, iid).unwrap();
        assert_eq!(
            get(&conn, ContainerKind::Milestone, rid)
                .unwrap()
                .unwrap()
                .status,
            ContainerStatus::Done
        );
    }

    /// 转移 plan 后源容器状态回落（源端重算，不再残留 running）。
    #[test]
    fn set_issue_plan_recomputes_source_plan() {
        let (conn, iid) = setup();
        let pid_a = create(&conn, ContainerKind::Plan, "a", None, None, None).unwrap();
        let pid_b = create(&conn, ContainerKind::Plan, "b", None, None, None).unwrap();
        set_issue_plan(&conn, iid, pid_a).unwrap();
        set_status(&conn, iid, "done");
        sync_container_status(&conn, iid).unwrap();
        assert_eq!(
            get(&conn, ContainerKind::Plan, pid_a)
                .unwrap()
                .unwrap()
                .status,
            ContainerStatus::Done
        );
        // 移到 plan B → A 无 issue 应回落 open，B 全 done
        set_issue_plan(&conn, iid, pid_b).unwrap();
        assert_eq!(
            get(&conn, ContainerKind::Plan, pid_a)
                .unwrap()
                .unwrap()
                .status,
            ContainerStatus::Open
        );
        assert_eq!(
            get(&conn, ContainerKind::Plan, pid_b)
                .unwrap()
                .unwrap()
                .status,
            ContainerStatus::Done
        );
    }

    /// 解绑直属 milestone 后源 milestone 状态回落。
    #[test]
    fn unlink_direct_recomputes_source_milestone() {
        let (conn, iid) = setup();
        let rid = create(
            &conn,
            ContainerKind::Milestone,
            "r",
            Some("0.1.0"),
            None,
            None,
        )
        .unwrap();
        link_direct(&conn, rid, iid).unwrap();
        set_status(&conn, iid, "done");
        sync_container_status(&conn, iid).unwrap();
        assert_eq!(
            get(&conn, ContainerKind::Milestone, rid)
                .unwrap()
                .unwrap()
                .status,
            ContainerStatus::Running
        );
        unlink_direct(&conn, rid, iid).unwrap();
        assert_eq!(
            get(&conn, ContainerKind::Milestone, rid)
                .unwrap()
                .unwrap()
                .status,
            ContainerStatus::Open
        );
    }

    /// detach 不存在的 milestone 报 not found（与 attach 校验对齐）。
    #[test]
    fn unlink_direct_missing_milestone_errors() {
        let (conn, iid) = setup();
        let err = unlink_direct(&conn, 999, iid).unwrap_err();
        assert!(
            err.to_string().contains("milestone #999 not found"),
            "err: {err}"
        );
    }

    /// detach 不存在的 issue 报 not found（#341：此前静默报成功）。
    #[test]
    fn unlink_direct_missing_issue_errors() {
        let (conn, _iid) = setup();
        let rid = create(
            &conn,
            ContainerKind::Milestone,
            "r",
            Some("0.1.0"),
            None,
            None,
        )
        .unwrap();
        let err = unlink_direct(&conn, rid, 999).unwrap_err();
        assert!(
            err.to_string().contains("issue #999 not found"),
            "err: {err}"
        );
    }

    /// unset_issue_plan 不存在的 issue 报 not found（#341）。
    #[test]
    fn unset_issue_plan_missing_issue_errors() {
        let (conn, _iid) = setup();
        let err = unset_issue_plan(&conn, 999).unwrap_err();
        assert!(
            err.to_string().contains("issue #999 not found"),
            "err: {err}"
        );
    }

    /// 二选一：issue 挂 plan 后不能再直接挂 milestone。
    #[test]
    fn plan_issue_cannot_direct_link() {
        let (conn, iid) = setup();
        let pid = create(&conn, ContainerKind::Plan, "p", None, None, None).unwrap();
        set_issue_plan(&conn, iid, pid).unwrap();
        let rid = create(
            &conn,
            ContainerKind::Milestone,
            "r",
            Some("0.1.0"),
            None,
            None,
        )
        .unwrap();
        let err = link_direct(&conn, rid, iid).unwrap_err();
        assert!(err.to_string().contains("already belongs to a plan"));
    }

    /// plan → milestone 级联：issue 变更同步 plan，再同步 milestone。
    #[test]
    fn issue_plan_milestone_cascade() {
        let (conn, iid) = setup();
        let rid = create(
            &conn,
            ContainerKind::Milestone,
            "r",
            Some("0.1.0"),
            None,
            None,
        )
        .unwrap();
        let pid = create(&conn, ContainerKind::Plan, "p", None, None, Some(rid)).unwrap();
        set_issue_plan(&conn, iid, pid).unwrap();

        set_status(&conn, iid, "done");
        sync_container_status(&conn, iid).unwrap();
        assert_eq!(
            get(&conn, ContainerKind::Plan, pid)
                .unwrap()
                .unwrap()
                .status,
            ContainerStatus::Done
        );
        assert_eq!(
            get(&conn, ContainerKind::Milestone, rid)
                .unwrap()
                .unwrap()
                .status,
            ContainerStatus::Running
        );
    }

    /// #223 修复：跨 milestone 移动 plan 时其下 planned issue 重置回 open（排期作废），
    /// plan 不再派生 running，旧侧派生回落、新侧按现状推进。
    #[test]
    fn move_plan_resets_planned_issues_and_derives() {
        let (conn, iid) = setup();
        let ms_a = create(
            &conn,
            ContainerKind::Milestone,
            "a",
            Some("0.5.0"),
            None,
            None,
        )
        .unwrap();
        let ms_b = create(
            &conn,
            ContainerKind::Milestone,
            "b",
            Some("2.0.0"),
            None,
            None,
        )
        .unwrap();
        let pid = create(&conn, ContainerKind::Plan, "p", None, None, Some(ms_a)).unwrap();
        set_issue_plan(&conn, iid, pid).unwrap();
        set_status(&conn, iid, "planned");
        sync_container_status(&conn, iid).unwrap();
        // 初始：issue planned → plan running → msA running（#223 现象）。
        assert_eq!(
            get(&conn, ContainerKind::Plan, pid)
                .unwrap()
                .unwrap()
                .status,
            ContainerStatus::Running
        );
        assert_eq!(
            get(&conn, ContainerKind::Milestone, ms_a)
                .unwrap()
                .unwrap()
                .status,
            ContainerStatus::Running
        );
        // 移到 msB（未来版本桶）：planned 重置 open，返回 1。
        let reset = move_plan(&conn, pid, ms_b).unwrap();
        assert_eq!(reset, 1);
        let st: String = conn
            .query_row(
                "SELECT status FROM issues WHERE id = ?1",
                params![iid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(st, "open");
        // plan 回落 open；旧侧 msA 回落 open；新侧 msB 派生 open（plan 全 open）。
        assert_eq!(
            get(&conn, ContainerKind::Plan, pid)
                .unwrap()
                .unwrap()
                .status,
            ContainerStatus::Open
        );
        assert_eq!(
            get(&conn, ContainerKind::Milestone, ms_a)
                .unwrap()
                .unwrap()
                .status,
            ContainerStatus::Open
        );
        assert_eq!(
            get(&conn, ContainerKind::Milestone, ms_b)
                .unwrap()
                .unwrap()
                .status,
            ContainerStatus::Open
        );
    }

    /// 只重置 planned：dev/test/done/dropped 保持，reset 计数精确。
    #[test]
    fn move_plan_keeps_non_planned_issues() {
        let (conn, iid) = setup();
        let ms_a = create(
            &conn,
            ContainerKind::Milestone,
            "a",
            Some("0.5.0"),
            None,
            None,
        )
        .unwrap();
        let ms_b = create(
            &conn,
            ContainerKind::Milestone,
            "b",
            Some("2.0.0"),
            None,
            None,
        )
        .unwrap();
        let pid = create(&conn, ContainerKind::Plan, "p", None, None, Some(ms_a)).unwrap();
        set_issue_plan(&conn, iid, pid).unwrap();
        set_status(&conn, iid, "planned");
        // 追加 dev/test/done/dropped 4 个 issue 挂同一 plan。
        for (t, st) in [("d", "dev"), ("t", "test"), ("n", "done"), ("r", "dropped")] {
            conn.execute("INSERT INTO issues (title) VALUES (?1)", params![t])
                .unwrap();
            let id = conn.last_insert_rowid();
            set_issue_plan(&conn, id, pid).unwrap();
            set_status(&conn, id, st);
        }
        let reset = move_plan(&conn, pid, ms_b).unwrap();
        assert_eq!(reset, 1, "仅 planned 被重置");
        let statuses: Vec<String> = conn
            .prepare("SELECT status FROM issues WHERE plan_id = ?1")
            .unwrap()
            .query_map(params![pid], |r| r.get(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            statuses,
            vec!["open", "dev", "test", "done", "dropped"],
            "planned→open，其余保持"
        );
    }

    /// 同 milestone 迁移 no-op：不重置排期。
    #[test]
    fn move_plan_same_milestone_is_noop() {
        let (conn, iid) = setup();
        let ms_a = create(
            &conn,
            ContainerKind::Milestone,
            "a",
            Some("0.5.0"),
            None,
            None,
        )
        .unwrap();
        let pid = create(&conn, ContainerKind::Plan, "p", None, None, Some(ms_a)).unwrap();
        set_issue_plan(&conn, iid, pid).unwrap();
        set_status(&conn, iid, "planned");
        sync_container_status(&conn, iid).unwrap();
        let reset = move_plan(&conn, pid, ms_a).unwrap();
        assert_eq!(reset, 0);
        let st: String = conn
            .query_row(
                "SELECT status FROM issues WHERE id = ?1",
                params![iid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(st, "planned");
    }

    /// 删除 plan：解绑其下 issue（plan_id 置 NULL），plan 消失、issue 保留。
    #[test]
    fn delete_plan_detaches_issues() {
        let (conn, iid) = setup();
        let pid = create(&conn, ContainerKind::Plan, "p", None, None, None).unwrap();
        set_issue_plan(&conn, iid, pid).unwrap();
        delete_plan(&conn, pid).unwrap();
        assert!(get(&conn, ContainerKind::Plan, pid).unwrap().is_none());
        let plan_id: Option<i64> = conn
            .query_row(
                "SELECT plan_id FROM issues WHERE id = ?1",
                params![iid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(plan_id, None);
    }

    /// 删除 milestone：直接挂 issue 解绑、其下 plan 保留（milestone_id 置 NULL）、milestone 消失。
    #[test]
    fn delete_milestone_detaches_plan_and_direct() {
        let (conn, iid) = setup();
        let rid = create(
            &conn,
            ContainerKind::Milestone,
            "r",
            Some("0.1.0"),
            None,
            None,
        )
        .unwrap();
        let pid = create(&conn, ContainerKind::Plan, "p", None, None, Some(rid)).unwrap();
        link_direct(&conn, rid, iid).unwrap();
        delete_milestone(&conn, rid).unwrap();
        assert!(get(&conn, ContainerKind::Milestone, rid).unwrap().is_none());
        let p = get(&conn, ContainerKind::Plan, pid).unwrap().unwrap();
        assert_eq!(p.milestone_id, None);
        let cnt: i64 = conn
            .query_row("SELECT COUNT(*) FROM milestone_direct_issues", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(cnt, 0);
    }

    /// 删除不存在对象报 not found。
    #[test]
    fn delete_missing_errors() {
        let (conn, _) = setup();
        let err = delete_plan(&conn, 999).unwrap_err();
        assert!(err.to_string().contains("plan #999 not found"), "{err}");
        let err = delete_milestone(&conn, 999).unwrap_err();
        assert!(
            err.to_string().contains("milestone #999 not found"),
            "{err}"
        );
        let err = delete_issue(&conn, 999).unwrap_err();
        assert!(err.to_string().contains("issue #999 not found"), "{err}");
    }

    /// 删除 plan 后，其上级 milestone 派生状态回落（plan 不再参与派生）。
    #[test]
    fn delete_plan_syncs_milestone_status() {
        let (conn, iid) = setup();
        let rid = create(
            &conn,
            ContainerKind::Milestone,
            "r",
            Some("0.1.0"),
            None,
            None,
        )
        .unwrap();
        let pid = create(&conn, ContainerKind::Plan, "p", None, None, Some(rid)).unwrap();
        set_issue_plan(&conn, iid, pid).unwrap();
        set_status(&conn, iid, "done");
        sync_container_status(&conn, iid).unwrap();
        assert_eq!(
            get(&conn, ContainerKind::Milestone, rid)
                .unwrap()
                .unwrap()
                .status,
            ContainerStatus::Running
        );
        delete_plan(&conn, pid).unwrap();
        assert_eq!(
            get(&conn, ContainerKind::Milestone, rid)
                .unwrap()
                .unwrap()
                .status,
            ContainerStatus::Open
        );
    }

    /// 移动 plan 到另一 milestone：两侧派生状态重算（旧侧回落、新侧推进），同一事务内原子。
    #[test]
    fn move_plan_syncs_both_milestones() {
        let (conn, iid) = setup();
        let aid = create(
            &conn,
            ContainerKind::Milestone,
            "a",
            Some("0.1.0"),
            None,
            None,
        )
        .unwrap();
        let bid = create(
            &conn,
            ContainerKind::Milestone,
            "b",
            Some("0.2.0"),
            None,
            None,
        )
        .unwrap();
        let pid = create(&conn, ContainerKind::Plan, "p", None, None, Some(aid)).unwrap();
        set_issue_plan(&conn, iid, pid).unwrap();
        set_status(&conn, iid, "done");
        sync_container_status(&conn, iid).unwrap();
        // plan 在 a 下且含 done issue → a 为 Running。
        assert_eq!(
            get(&conn, ContainerKind::Milestone, aid)
                .unwrap()
                .unwrap()
                .status,
            ContainerStatus::Running
        );
        // 移到 b → a 回落 Open、b 推进 Running、plan 归属更新。
        move_plan(&conn, pid, bid).unwrap();
        assert_eq!(
            get(&conn, ContainerKind::Milestone, aid)
                .unwrap()
                .unwrap()
                .status,
            ContainerStatus::Open
        );
        assert_eq!(
            get(&conn, ContainerKind::Milestone, bid)
                .unwrap()
                .unwrap()
                .status,
            ContainerStatus::Running
        );
        assert_eq!(
            get(&conn, ContainerKind::Plan, pid)
                .unwrap()
                .unwrap()
                .milestone_id,
            Some(bid)
        );
    }

    /// 移到空 milestone：新侧无子项 → Open。
    #[test]
    fn move_plan_to_empty_milestone() {
        let (conn, _) = setup();
        let aid = create(
            &conn,
            ContainerKind::Milestone,
            "a",
            Some("0.1.0"),
            None,
            None,
        )
        .unwrap();
        let bid = create(
            &conn,
            ContainerKind::Milestone,
            "b",
            Some("0.2.0"),
            None,
            None,
        )
        .unwrap();
        let pid = create(&conn, ContainerKind::Plan, "p", None, None, Some(aid)).unwrap();
        move_plan(&conn, pid, bid).unwrap();
        assert_eq!(
            get(&conn, ContainerKind::Milestone, bid)
                .unwrap()
                .unwrap()
                .status,
            ContainerStatus::Open
        );
    }

    /// plan 不存在 → 报错。
    #[test]
    fn move_plan_not_found() {
        let (conn, _) = setup();
        let bid = create(
            &conn,
            ContainerKind::Milestone,
            "b",
            Some("0.2.0"),
            None,
            None,
        )
        .unwrap();
        let err = move_plan(&conn, 999, bid).unwrap_err();
        assert!(err.to_string().contains("plan #999 not found"), "{err}");
    }

    /// 目标 milestone 不存在 → 报错。
    #[test]
    fn move_plan_missing_milestone() {
        let (conn, _) = setup();
        let aid = create(
            &conn,
            ContainerKind::Milestone,
            "a",
            Some("0.1.0"),
            None,
            None,
        )
        .unwrap();
        let pid = create(&conn, ContainerKind::Plan, "p", None, None, Some(aid)).unwrap();
        let err = move_plan(&conn, pid, 999).unwrap_err();
        assert!(
            err.to_string().contains("milestone #999 not found"),
            "{err}"
        );
    }

    /// 同 milestone 迁移：no-op，状态不变。
    #[test]
    fn move_plan_same_milestone_noop() {
        let (conn, _) = setup();
        let aid = create(
            &conn,
            ContainerKind::Milestone,
            "a",
            Some("0.1.0"),
            None,
            None,
        )
        .unwrap();
        let pid = create(&conn, ContainerKind::Plan, "p", None, None, Some(aid)).unwrap();
        move_plan(&conn, pid, aid).unwrap();
        assert_eq!(
            get(&conn, ContainerKind::Milestone, aid)
                .unwrap()
                .unwrap()
                .status,
            ContainerStatus::Open
        );
    }

    /// update_plan 更新 title，body 不变。
    #[test]
    fn update_plan_changes_title() {
        let (conn, _) = setup();
        let pid = create(
            &conn,
            ContainerKind::Plan,
            "old",
            None,
            Some("old body"),
            None,
        )
        .unwrap();
        update_plan(&conn, pid, Some("new title"), None).unwrap();
        let p = get(&conn, ContainerKind::Plan, pid).unwrap().unwrap();
        assert_eq!(p.title, "new title");
        assert_eq!(p.body.as_deref(), Some("old body"));
    }

    /// update_plan 更新 body，title 不变。
    #[test]
    fn update_plan_preserves_title() {
        let (conn, _) = setup();
        let pid = create(
            &conn,
            ContainerKind::Plan,
            "t",
            None,
            Some("old body"),
            None,
        )
        .unwrap();
        update_plan(&conn, pid, None, Some("new body")).unwrap();
        let p = get(&conn, ContainerKind::Plan, pid).unwrap().unwrap();
        assert_eq!(p.title, "t");
        assert_eq!(p.body.as_deref(), Some("new body"));
    }

    /// update_plan 不存在的 plan 报 not found。
    #[test]
    fn update_plan_missing_errors() {
        let (conn, _) = setup();
        let err = update_plan(&conn, 999, Some("x"), None).unwrap_err();
        assert!(err.to_string().contains("plan #999 not found"), "{err}");
    }

    /// update_milestone 更新 version，title/body 不变。
    #[test]
    fn update_milestone_changes_version() {
        let (conn, _) = setup();
        let rid = create(
            &conn,
            ContainerKind::Milestone,
            "r",
            Some("0.1.0"),
            Some("old body"),
            None,
        )
        .unwrap();
        update_milestone(&conn, rid, None, Some("0.2.0"), None).unwrap();
        let r = get(&conn, ContainerKind::Milestone, rid).unwrap().unwrap();
        assert_eq!(r.version, Some("0.2.0".to_string()));
        assert_eq!(r.title, "r");
        assert_eq!(r.body.as_deref(), Some("old body"));
    }

    /// update_milestone 不存在的 milestone 报 not found。
    #[test]
    fn update_milestone_missing_errors() {
        let (conn, _) = setup();
        let err = update_milestone(&conn, 999, Some("x"), None, None).unwrap_err();
        assert!(
            err.to_string().contains("milestone #999 not found"),
            "{err}"
        );
    }

    /// 物理删除属容器的 issue 后，父容器派生状态回落（done → open）。
    #[test]
    fn delete_issue_syncs_plan_status() {
        let (conn, iid) = setup();
        let pid = create(&conn, ContainerKind::Plan, "p", None, None, None).unwrap();
        set_issue_plan(&conn, iid, pid).unwrap();
        set_status(&conn, iid, "done");
        sync_container_status(&conn, iid).unwrap();
        assert_eq!(
            get(&conn, ContainerKind::Plan, pid)
                .unwrap()
                .unwrap()
                .status,
            ContainerStatus::Done
        );
        delete_issue(&conn, iid).unwrap();
        assert_eq!(
            get(&conn, ContainerKind::Plan, pid)
                .unwrap()
                .unwrap()
                .status,
            ContainerStatus::Open
        );
    }
}
