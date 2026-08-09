//! dashboard 数据加载：全量快照（全部 issue + 全部 plan，无 milestone）。

use rusqlite::Connection;

use crate::cli::issue::list::{fill_labels, issue_from_row};
use crate::container::{self, ContainerKind};
use crate::db;
use crate::error::Error;
use crate::models::{Issue, Status};
use crate::tui::dashboard_diff::DashboardSnapshot;

/// 全量快照：全部 issue（含 labels）+ 全部 plan（含直接挂载 issue 计数）。
pub fn load_snapshot(conn: &Connection) -> Result<DashboardSnapshot, Error> {
    let mut stmt = conn.prepare(db::ISSUE_LIST)?;
    let rows = stmt.query_map(
        rusqlite::params![
            1,
            None::<Status>,
            None::<String>,
            None::<String>,
            None::<i64>
        ],
        issue_from_row,
    )?;
    let mut issues: Vec<Issue> = rows.collect::<Result<_, _>>()?;
    fill_labels(conn, &mut issues)?;
    let plans = container::list(conn, ContainerKind::Plan, true)?;
    Ok(DashboardSnapshot { issues, plans })
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

        let snap = load_snapshot(&conn).unwrap();
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
    }

    #[test]
    fn snapshot_empty_db() {
        let conn = db();
        let snap = load_snapshot(&conn).unwrap();
        assert!(snap.issues.is_empty());
        assert!(snap.plans.is_empty());
    }
}
