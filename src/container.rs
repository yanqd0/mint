//! 容器（roadmap/plan）共享模块：聚合 issue/plan。
//!
//! 层级：roadmap → plan（roadmap_id）→ issue（plan_id）；roadmap 可直接挂无 plan 的 issue
//! （roadmap_direct_issues，二选一约束）。容器状态 5 态派生（写后同步，CLI 只读）。

use rusqlite::{Connection, OptionalExtension, params};

use crate::db;
use crate::error::Error;
use crate::models::{Container, ContainerStatus, IssueSummary, Status};

/// 容器类型：roadmap / plan。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerKind {
    Roadmap,
    Plan,
}

impl ContainerKind {
    fn insert_sql(self) -> &'static str {
        match self {
            ContainerKind::Roadmap => db::ROADMAP_INSERT,
            ContainerKind::Plan => db::PLAN_INSERT,
        }
    }

    fn list_sql(self) -> &'static str {
        match self {
            ContainerKind::Roadmap => db::ROADMAP_LIST,
            ContainerKind::Plan => db::PLAN_LIST,
        }
    }

    fn select_sql(self) -> &'static str {
        match self {
            ContainerKind::Roadmap => db::ROADMAP_SELECT,
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

/// 新建容器，返回 id。roadmap 必填 version；plan 可带 roadmap_id。
pub fn create(
    conn: &Connection,
    kind: ContainerKind,
    title: &str,
    version: Option<&str>,
    body: Option<&str>,
    roadmap_id: Option<i64>,
) -> Result<i64, Error> {
    match kind {
        ContainerKind::Roadmap => {
            let v = version.filter(|s| !s.trim().is_empty());
            if v.is_none() {
                return Err(Error::Other("roadmap requires --version".to_string()));
            }
            conn.execute(kind.insert_sql(), params![title, v, body])?;
        }
        ContainerKind::Plan => {
            conn.execute(kind.insert_sql(), params![title, body, roadmap_id])?;
        }
    }
    Ok(conn.last_insert_rowid())
}

/// 列出全部容器（含子项计数），(容器, 计数)。
pub fn list(
    conn: &Connection,
    kind: ContainerKind,
    all: bool,
) -> Result<Vec<(Container, i64)>, Error> {
    let all_flag: i64 = if all { 1 } else { 0 };
    let mut stmt = conn.prepare(kind.list_sql())?;
    let rows = stmt.query_map(params![all_flag], |r| {
        let container = Container {
            id: r.get(0)?,
            title: r.get(1)?,
            version: r.get(2)?,
            body: r.get(3)?,
            roadmap_id: r.get(4)?,
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
            roadmap_id: r.get(4)?,
            status: r.get(5)?,
            created_at: r.get(6)?,
            updated_at: r.get(7)?,
        })
    })
    .optional()
    .map_err(Error::from)
}

/// 查询容器下的 issue 摘要。
pub fn issues_for(
    conn: &Connection,
    kind: ContainerKind,
    id: i64,
) -> Result<Vec<IssueSummary>, Error> {
    let sql = match kind {
        ContainerKind::Roadmap => db::ROADMAP_ISSUES_FOR,
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

/// roadmap 直接挂 issue（仅接受无 plan 的 issue）。幂等。
pub fn link_direct(conn: &Connection, roadmap_id: i64, issue_id: i64) -> Result<(), Error> {
    if get(conn, ContainerKind::Roadmap, roadmap_id)?.is_none() {
        return Err(Error::Other(format!("roadmap #{roadmap_id} not found")));
    }
    let plan_id: Option<Option<i64>> = conn
        .query_row(
            "SELECT plan_id FROM issues WHERE id = ?1",
            params![issue_id],
            |r| r.get(0),
        )
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
    conn.execute(db::ROADMAP_ATTACH, params![roadmap_id, issue_id])?;
    sync_container_status(conn, issue_id)?;
    Ok(())
}

/// 解除 roadmap 直接挂的 issue。无行静默 no-op。
pub fn unlink_direct(conn: &Connection, roadmap_id: i64, issue_id: i64) -> Result<(), Error> {
    conn.execute(db::ROADMAP_DETACH, params![roadmap_id, issue_id])?;
    sync_container_status(conn, issue_id)?;
    Ok(())
}

/// 把 issue 挂到 plan 下（plan_id 外键）。plan 不存在报错。
pub fn set_issue_plan(conn: &Connection, issue_id: i64, plan_id: i64) -> Result<(), Error> {
    if get(conn, ContainerKind::Plan, plan_id)?.is_none() {
        return Err(Error::Other(format!("plan #{plan_id} not found")));
    }
    let exists: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM issues WHERE id = ?1",
            params![issue_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(Error::from)?;
    if exists.is_none() {
        return Err(Error::Other(format!("issue #{issue_id} not found")));
    }
    // 若该 issue 已直接挂 roadmap，需先解除（二选一）
    conn.execute(
        "DELETE FROM roadmap_direct_issues WHERE issue_id = ?1",
        params![issue_id],
    )?;
    conn.execute(
        "UPDATE issues SET plan_id = ?1, updated_at = datetime('now') WHERE id = ?2",
        params![plan_id, issue_id],
    )?;
    sync_container_status(conn, issue_id)?;
    Ok(())
}

/// 解除 issue 的 plan 归属（plan_id 置 NULL）。
pub fn unset_issue_plan(conn: &Connection, issue_id: i64) -> Result<(), Error> {
    conn.execute(
        "UPDATE issues SET plan_id = NULL, updated_at = datetime('now') WHERE id = ?1",
        params![issue_id],
    )?;
    sync_container_status(conn, issue_id)?;
    Ok(())
}

/// 写后级联同步：某 issue 状态/归属变化后，重算其所属 plan 与 roadmap 的状态。
/// 事务由调用方保证（transition 等写路径在事务内调用）。
pub fn sync_container_status(conn: &Connection, issue_id: i64) -> Result<(), Error> {
    // plan 同步
    let plan_ids: Vec<i64> = conn
        .prepare(db::PLAN_IDS_FOR_ISSUE)
        .unwrap()
        .query_map(params![issue_id], |r| r.get(0))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(Error::from)?;
    for pid in plan_ids {
        sync_plan(conn, pid)?;
    }
    // roadmap 直接挂的同步
    let r_ids: Vec<i64> = conn
        .prepare(db::ROADMAP_IDS_FOR_ISSUE)
        .unwrap()
        .query_map(params![issue_id], |r| r.get(0))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(Error::from)?;
    for rid in r_ids {
        sync_roadmap(conn, rid)?;
    }
    Ok(())
}

/// 重算某 plan 状态并写回；随后同步其所属 roadmap。
fn sync_plan(conn: &Connection, plan_id: i64) -> Result<(), Error> {
    let statuses = issue_statuses_from(conn, db::PLAN_ISSUE_STATUSES, plan_id)?;
    let st = derive_status(&statuses);
    conn.execute(db::PLAN_UPDATE_STATUS, params![st, plan_id])?;
    // plan 变更 → 检查 roadmap
    let roadmap_ids: Vec<i64> = conn
        .prepare(db::ROADMAP_IDS_FOR_PLAN)
        .unwrap()
        .query_map(params![plan_id], |r| r.get(0))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(Error::from)?;
    for rid in roadmap_ids {
        sync_roadmap(conn, rid)?;
    }
    Ok(())
}

/// 重算某 roadmap 状态（plan 状态 + 直接挂 issue 状态合并）并写回。
fn sync_roadmap(conn: &Connection, roadmap_id: i64) -> Result<(), Error> {
    let mut statuses = container_statuses_from(conn, db::ROADMAP_PLAN_STATUSES, roadmap_id)?;
    statuses.extend(issue_statuses_from(
        conn,
        db::ROADMAP_DIRECT_ISSUE_STATUSES,
        roadmap_id,
    )?);
    let st = derive_status(&statuses);
    conn.execute(db::ROADMAP_UPDATE_STATUS, params![st, roadmap_id])?;
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

    fn setup() -> (Connection, i64) {
        let conn = db::open(std::path::Path::new(":memory:")).unwrap();
        conn.execute("INSERT INTO projects (name) VALUES ('p')", [])
            .unwrap();
        let pid: i64 = conn
            .query_row("SELECT id FROM projects WHERE name='p'", [], |r| r.get(0))
            .unwrap();
        conn.execute(
            "INSERT INTO issues (title, project_id) VALUES ('a', ?1)",
            params![pid],
        )
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

    /// derive_status 全分支。
    #[test]
    fn derive_status_rules() {
        use DeriveState::{Active, Done, Dropped, Open};
        assert_eq!(derive_status(&[]), ContainerStatus::Open);
        assert_eq!(derive_status(&[Open, Open]), ContainerStatus::Open);
        assert_eq!(derive_status(&[Active]), ContainerStatus::Running);
        assert_eq!(derive_status(&[Active, Done]), ContainerStatus::Running);
        assert_eq!(derive_status(&[Done, Done]), ContainerStatus::Done);
        assert_eq!(derive_status(&[Dropped, Dropped]), ContainerStatus::Dropped);
        assert_eq!(derive_status(&[Done, Dropped]), ContainerStatus::Partial);
        // 有非 open 未全 done/dropped → running（曾运行）
        assert_eq!(derive_status(&[Open, Done]), ContainerStatus::Running);
        assert_eq!(derive_status(&[Open, Dropped]), ContainerStatus::Running);
        assert_eq!(
            derive_status(&[Open, Done, Dropped]),
            ContainerStatus::Running
        );
    }

    /// roadmap create 必填 version。
    #[test]
    fn create_requires_version() {
        let (conn, _) = setup();
        let err = create(&conn, ContainerKind::Roadmap, "r", None, None, None).unwrap_err();
        assert!(err.to_string().contains("--version"));
        let id = create(
            &conn,
            ContainerKind::Roadmap,
            "r",
            Some("0.1.0"),
            None,
            None,
        )
        .unwrap();
        assert!(id > 0);
    }

    /// roadmap 直接挂 issue（无 plan）+ 派生状态同步。
    #[test]
    fn roadmap_direct_issue_derives_status() {
        let (conn, iid) = setup();
        let rid = create(
            &conn,
            ContainerKind::Roadmap,
            "r",
            Some("0.1.0"),
            None,
            None,
        )
        .unwrap();
        link_direct(&conn, rid, iid).unwrap();
        assert_eq!(
            get(&conn, ContainerKind::Roadmap, rid)
                .unwrap()
                .unwrap()
                .status,
            ContainerStatus::Open
        );

        set_status(&conn, iid, "done");
        sync_container_status(&conn, iid).unwrap();
        assert_eq!(
            get(&conn, ContainerKind::Roadmap, rid)
                .unwrap()
                .unwrap()
                .status,
            ContainerStatus::Done
        );
    }

    /// 二选一：issue 挂 plan 后不能再直接挂 roadmap。
    #[test]
    fn plan_issue_cannot_direct_link() {
        let (conn, iid) = setup();
        let pid = create(&conn, ContainerKind::Plan, "p", None, None, None).unwrap();
        set_issue_plan(&conn, iid, pid).unwrap();
        let rid = create(
            &conn,
            ContainerKind::Roadmap,
            "r",
            Some("0.1.0"),
            None,
            None,
        )
        .unwrap();
        let err = link_direct(&conn, rid, iid).unwrap_err();
        assert!(err.to_string().contains("already belongs to a plan"));
    }

    /// plan → roadmap 级联：issue 变更同步 plan，再同步 roadmap。
    #[test]
    fn issue_plan_roadmap_cascade() {
        let (conn, iid) = setup();
        let rid = create(
            &conn,
            ContainerKind::Roadmap,
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
            get(&conn, ContainerKind::Roadmap, rid)
                .unwrap()
                .unwrap()
                .status,
            ContainerStatus::Done
        );
    }
}
