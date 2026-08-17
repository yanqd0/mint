//! dashboard 数据加载：全量快照（全部 issue + 全部 plan + 全部 milestone）。

use rusqlite::Connection;

use crate::cli::issue::list::{fill_labels, issue_from_row};
use crate::container::{self, ContainerKind};
use crate::db;
use crate::error::Error;
use crate::label;
use crate::link;
use crate::models::{Issue, Status};
use crate::tui::dashboard::diff::DashboardSnapshot;

/// 全量快照：当前项目 issue（含 labels）+ 全部 plan + 全部 milestone。
pub fn load_snapshot(conn: &Connection, project: &str) -> Result<DashboardSnapshot, Error> {
    let mut stmt = conn.prepare(db::ISSUE_LIST)?;
    let rows = stmt.query_map(
        rusqlite::params![
            1,
            None::<Status>,
            None::<String>,
            Some(project),
            None::<i64>
        ],
        issue_from_row,
    )?;
    let mut issues: Vec<Issue> = rows.collect::<Result<_, _>>()?;
    fill_labels(conn, &mut issues)?;
    // 补 label 名 → color 映射（TUI 渲染着色用）。
    let colors_map = label::colors_for_issues(conn)?;
    for i in issues.iter_mut() {
        i.label_colors = colors_map
            .get(&i.id)
            .map(|pairs| pairs.iter().cloned().collect())
            .unwrap_or_default();
    }
    // 补 links（出向 + 入向反向派生），供详情页 links 列表与 diff 使用；批量一次取回防 N+1。
    let links_map = link::links_for_many(conn)?;
    for i in issues.iter_mut() {
        i.links = links_map.get(&i.id).cloned().unwrap_or_default();
    }
    let plans = container::list(conn, ContainerKind::Plan, true, None)?;
    let milestones = container::list(conn, ContainerKind::Milestone, true, None)?;
    // milestone 直属 issue 关联（详情页直属 issue 列表用）。
    let milestone_directs = {
        let mut stmt = conn.prepare(db::MILESTONE_DIRECTS_ALL)?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    Ok(DashboardSnapshot {
        issues,
        plans,
        milestones,
        project: project.to_string(),
        milestone_directs,
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn db() -> Connection {
        let conn = crate::db::open(Path::new(":memory:")).unwrap();
        conn.execute("INSERT INTO projects (name) VALUES ('mint')", [])
            .unwrap();
        conn
    }

    #[test]
    fn snapshot_collects_issues_and_plans() {
        let conn = db();
        conn.execute(
            "INSERT INTO issues (title, kind, status, priority, project_id)
             VALUES ('a', 'problem', 'open', 3, 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO issues (title, kind, status, priority, project_id)
             VALUES ('b', 'requirement', 'done', 2, 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO plans (title, status) VALUES ('sprint', 'open')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO milestones (title, version, status) VALUES ('0.5.0', '0.5.0', 'open')",
            [],
        )
        .unwrap();

        let snap = load_snapshot(&conn, "mint").unwrap();
        assert_eq!(snap.issues.len(), 2);
        let a = snap.issue(1).unwrap();
        assert_eq!(a.title, "a");
        assert_eq!(a.status, Status::Open);
        let b = snap.issue(2).unwrap();
        assert_eq!(b.title, "b");
        assert_eq!(b.status, Status::Done);
        assert_eq!(snap.plans.len(), 1);
        assert_eq!(snap.plans[0].0.title, "sprint");
        // 直接挂载计数：plan 无直接 issue → 0
        assert_eq!(snap.plans[0].1, 0);
        assert_eq!(snap.milestones.len(), 1);
        assert_eq!(snap.milestones[0].0.title, "0.5.0");
    }

    #[test]
    fn snapshot_empty_db() {
        let conn = db();
        let snap = load_snapshot(&conn, "mint").unwrap();
        assert!(snap.issues.is_empty());
        assert!(snap.plans.is_empty());
    }

    #[test]
    fn snapshot_fills_issue_links() {
        let conn = db();
        conn.execute(
            "INSERT INTO issues (title, kind, status, priority, project_id)
             VALUES ('a', 'problem', 'open', 3, 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO issues (title, kind, status, priority, project_id)
             VALUES ('b', 'problem', 'open', 3, 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO issue_links (from_id, type, to_id) VALUES (1, 'related', 2)",
            [],
        )
        .unwrap();
        let snap = load_snapshot(&conn, "mint").unwrap();
        let a = snap.issue(1).unwrap();
        assert_eq!(a.links.len(), 1);
        assert_eq!(a.links[0].other_id, 2);
        assert_eq!(a.links[0].rel, "related");
        // 入向反向派生：issue 2 也聚合到 link（related 对称，rel 仍为 related）。
        let b = snap.issue(2).unwrap();
        assert_eq!(b.links.len(), 1);
        assert_eq!(b.links[0].other_id, 1);
        assert_eq!(b.links[0].rel, "related");
    }
}
