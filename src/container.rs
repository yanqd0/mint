//! 容器（roadmap/plan）共享模块：聚合多个 issue。
//!
//! roadmap 与 plan 同构（ContainerKind 分发到不同 SQL 常量），一次设计。
//! 关联表复用 issue_tags 的复合主键 + INSERT OR IGNORE 模式。

use rusqlite::{Connection, OptionalExtension, params};

use crate::db;
use crate::error::Error;
use crate::models::{Container, ContainerStatus, IssueSummary};

/// 容器类型：roadmap / plan。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerKind {
    Roadmap,
    Plan,
}

impl ContainerKind {
    fn noun(self) -> &'static str {
        match self {
            ContainerKind::Roadmap => "roadmap",
            ContainerKind::Plan => "plan",
        }
    }

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

    fn attach_sql(self) -> &'static str {
        match self {
            ContainerKind::Roadmap => db::ROADMAP_ATTACH,
            ContainerKind::Plan => db::PLAN_ATTACH,
        }
    }

    fn detach_sql(self) -> &'static str {
        match self {
            ContainerKind::Roadmap => db::ROADMAP_DETACH,
            ContainerKind::Plan => db::PLAN_DETACH,
        }
    }

    fn issues_for_sql(self) -> &'static str {
        match self {
            ContainerKind::Roadmap => db::ROADMAP_ISSUES_FOR,
            ContainerKind::Plan => db::PLAN_ISSUES_FOR,
        }
    }

    fn select_status_sql(self) -> &'static str {
        match self {
            ContainerKind::Roadmap => db::ROADMAP_SELECT_STATUS,
            ContainerKind::Plan => db::PLAN_SELECT_STATUS,
        }
    }

    fn update_status_sql(self) -> &'static str {
        match self {
            ContainerKind::Roadmap => db::ROADMAP_UPDATE_STATUS,
            ContainerKind::Plan => db::PLAN_UPDATE_STATUS,
        }
    }
}

/// 新建容器，返回 id（status=open）。
pub fn create(
    conn: &Connection,
    kind: ContainerKind,
    title: &str,
    description: Option<&str>,
) -> Result<i64, Error> {
    conn.execute(kind.insert_sql(), params![title, description])?;
    Ok(conn.last_insert_rowid())
}

/// 列出全部容器（含关联 issue 计数），(容器, 计数)。
pub fn list(conn: &Connection, kind: ContainerKind) -> Result<Vec<(Container, i64)>, Error> {
    let mut stmt = conn.prepare(kind.list_sql())?;
    let rows = stmt.query_map([], |r| {
        let container = Container {
            id: r.get(0)?,
            title: r.get(1)?,
            description: r.get(2)?,
            status: r.get(3)?,
            dropped_reason: r.get(4)?,
            created_at: r.get(5)?,
            updated_at: r.get(6)?,
        };
        let count: i64 = r.get(7)?;
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
            description: r.get(2)?,
            status: r.get(3)?,
            dropped_reason: r.get(4)?,
            created_at: r.get(5)?,
            updated_at: r.get(6)?,
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
    let mut stmt = conn.prepare(kind.issues_for_sql())?;
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

/// 为容器关联 issue（幂等）。校验两端存在。
pub fn link(
    conn: &Connection,
    kind: ContainerKind,
    container_id: i64,
    issue_id: i64,
) -> Result<(), Error> {
    if get(conn, kind, container_id)?.is_none() {
        return Err(Error::Other(format!(
            "{} #{container_id} not found",
            kind.noun()
        )));
    }
    let exists: Option<String> = conn
        .query_row(db::ISSUE_SELECT_STATUS, params![issue_id], |r| r.get(0))
        .optional()
        .map_err(Error::from)?;
    if exists.is_none() {
        return Err(Error::Other(format!("issue #{issue_id} not found")));
    }
    conn.execute(kind.attach_sql(), params![container_id, issue_id])?;
    Ok(())
}

/// 解除容器与 issue 的关联（无行则静默 no-op）。
pub fn unlink(
    conn: &Connection,
    kind: ContainerKind,
    container_id: i64,
    issue_id: i64,
) -> Result<(), Error> {
    conn.execute(kind.detach_sql(), params![container_id, issue_id])?;
    Ok(())
}

/// 容器状态转换动作（close/drop/reopen）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerAction {
    Close,
    Drop,
    Reopen,
}

/// 动作的目标状态。
pub fn container_target_of(a: ContainerAction) -> ContainerStatus {
    match a {
        ContainerAction::Close => ContainerStatus::Done,
        ContainerAction::Drop => ContainerStatus::Dropped,
        ContainerAction::Reopen => ContainerStatus::Open,
    }
}

/// 校验 `action` 能否把 `current` 推进到 `target`。
pub fn container_can_transition(
    current: ContainerStatus,
    a: ContainerAction,
    target: ContainerStatus,
) -> bool {
    target == container_target_of(a)
        && match a {
            ContainerAction::Close => current == ContainerStatus::Open,
            ContainerAction::Drop => true,
            ContainerAction::Reopen => {
                matches!(current, ContainerStatus::Done | ContainerStatus::Dropped)
            }
        }
}

/// 容器状态转换：读当前 → 校验 → 更新，返回 (from, to)。
pub fn transition(
    conn: &Connection,
    kind: ContainerKind,
    id: i64,
    action: ContainerAction,
    reason: Option<&str>,
) -> Result<(ContainerStatus, ContainerStatus), Error> {
    let current: ContainerStatus = conn
        .query_row(kind.select_status_sql(), params![id], |r| r.get(0))
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                Error::Other(format!("{} #{id} not found", kind.noun()))
            }
            other => Error::from(other),
        })?;

    let target = container_target_of(action);
    if !container_can_transition(current, action, target) {
        return Err(Error::Other(format!(
            "invalid transition: {} -> {} via {:?}",
            current, target, action
        )));
    }

    // drop 写 reason；reopen 清空 dropped_reason（与 issue 对称）
    let drop_reason: Option<&str> = if action == ContainerAction::Drop {
        reason
    } else {
        None
    };
    let reopen = action == ContainerAction::Reopen;
    conn.execute(
        kind.update_status_sql(),
        params![target, id, drop_reason, reopen],
    )?;

    Ok((current, target))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::models::ContainerStatus;

    fn setup() -> (Connection, i64) {
        let conn = db::open(std::path::Path::new(":memory:")).unwrap();
        // 注册 project + 一个 issue
        conn.execute("INSERT INTO projects (name) VALUES ('p')", [])
            .unwrap();
        let pid: i64 = conn
            .query_row("SELECT id FROM projects WHERE name='p'", [], |r| r.get(0))
            .unwrap();
        conn.execute(
            "INSERT INTO issues (title, project_id) VALUES ('issue', ?1)",
            params![pid],
        )
        .unwrap();
        let iid: i64 = conn
            .query_row("SELECT id FROM issues", [], |r| r.get(0))
            .unwrap();
        (conn, iid)
    }

    /// create 后 list 计数、get 可取回。
    #[test]
    fn create_list_get_works() {
        let (conn, _) = setup();
        let id = create(&conn, ContainerKind::Roadmap, "0.2.0", Some("容器")).unwrap();
        assert!(id > 0);

        let items = list(&conn, ContainerKind::Roadmap).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].0.title, "0.2.0");
        assert_eq!(items[0].1, 0, "无关联 issue 计数为 0");

        let c = get(&conn, ContainerKind::Roadmap, id).unwrap().unwrap();
        assert_eq!(c.status, ContainerStatus::Open);
    }

    /// link 幂等、unlink 解除。
    #[test]
    fn link_unlink_idempotent() {
        let (conn, iid) = setup();
        let id = create(&conn, ContainerKind::Plan, "plan", None).unwrap();

        link(&conn, ContainerKind::Plan, id, iid).unwrap();
        link(&conn, ContainerKind::Plan, id, iid).unwrap(); // 幂等
        let issues = issues_for(&conn, ContainerKind::Plan, id).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].title, "issue");

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM plan_issues", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);

        unlink(&conn, ContainerKind::Plan, id, iid).unwrap();
        let issues = issues_for(&conn, ContainerKind::Plan, id).unwrap();
        assert_eq!(issues.len(), 0);
    }

    /// link 不存在的容器/issue 报错。
    #[test]
    fn link_missing_ids_errors() {
        let (conn, iid) = setup();
        let err = link(&conn, ContainerKind::Roadmap, 999, iid).unwrap_err();
        assert!(err.to_string().contains("roadmap #999 not found"));

        let id = create(&conn, ContainerKind::Roadmap, "r", None).unwrap();
        let err = link(&conn, ContainerKind::Roadmap, id, 999).unwrap_err();
        assert!(err.to_string().contains("issue #999 not found"));
    }

    /// 状态转换：close→done、drop 写 reason、reopen 清 dropped_reason。
    #[test]
    fn status_transitions() {
        let (conn, _) = setup();
        let id = create(&conn, ContainerKind::Roadmap, "r", None).unwrap();

        let (from, to) = transition(
            &conn,
            ContainerKind::Roadmap,
            id,
            ContainerAction::Close,
            None,
        )
        .unwrap();
        assert_eq!(from, ContainerStatus::Open);
        assert_eq!(to, ContainerStatus::Done);

        // done 上不能 close（已 done）
        let err = transition(
            &conn,
            ContainerKind::Roadmap,
            id,
            ContainerAction::Close,
            None,
        )
        .unwrap_err();
        assert!(err.to_string().contains("invalid transition"));

        // reopen 回 open，清 dropped_reason
        let _ = transition(
            &conn,
            ContainerKind::Roadmap,
            id,
            ContainerAction::Drop,
            Some("obsolete"),
        )
        .unwrap();
        let c = get(&conn, ContainerKind::Roadmap, id).unwrap().unwrap();
        assert_eq!(c.status, ContainerStatus::Dropped);
        assert_eq!(c.dropped_reason.as_deref(), Some("obsolete"));

        let (from, to) = transition(
            &conn,
            ContainerKind::Roadmap,
            id,
            ContainerAction::Reopen,
            None,
        )
        .unwrap();
        assert_eq!(from, ContainerStatus::Dropped);
        assert_eq!(to, ContainerStatus::Open);
        let c = get(&conn, ContainerKind::Roadmap, id).unwrap().unwrap();
        assert_eq!(c.status, ContainerStatus::Open);
        assert_eq!(c.dropped_reason, None);
    }

    /// 非法转换拒绝（open 直接 reopen）。
    #[test]
    fn illegal_transition_rejected() {
        let (conn, _) = setup();
        let id = create(&conn, ContainerKind::Plan, "p", None).unwrap();
        let err = transition(
            &conn,
            ContainerKind::Plan,
            id,
            ContainerAction::Reopen,
            None,
        )
        .unwrap_err();
        assert!(err.to_string().contains("invalid transition"));
    }
}
