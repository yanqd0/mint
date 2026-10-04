//! dashboard 状态机测试：原 model_tests.rs 按主题拆分为子模块，共享构造 helper 见下。

use super::*;
use crate::models::{ContainerStatus, Kind, Status};
use crate::tui::dashboard::diff::DashboardSnapshot;

mod cursor;
mod detail;
mod nav;
mod paging;
mod search;
mod search_commit;
mod select;
mod tabs;

/// 无修饰符按键快捷构造（handle_key 改收 TuiKey 后测试用）。
fn k(code: KeyCode) -> TuiKey {
    TuiKey::from_code(code)
}

fn mk_issue(id: i64, status: Status, plan_id: Option<i64>, updated: &str) -> Issue {
    Issue {
        id,
        title: "t".into(),
        body: None,
        kind: Kind::Problem,
        status,
        priority: 3,
        project: Some("mint".into()),
        test_cmd: None,
        dropped_reason: None,
        last_commit_id: None,
        plan_id,
        direct_milestone: None,
        machine_id: None,
        uid: None,
        hit_count: 0,
        label_colors: std::collections::HashMap::new(),
        labels: vec![],
        links: vec![],
        created_at: "t".into(),
        updated_at: updated.into(),
    }
}

fn mk_container(id: i64) -> Container {
    Container {
        id,
        title: "p".into(),
        version: None,
        body: None,
        milestone_id: None,
        status: ContainerStatus::Open,
        created_at: "t".into(),
        updated_at: "t".into(),
        sort_order: None,
    }
}

fn mk_plan(id: i64, milestone: Option<i64>, updated: &str) -> Container {
    Container {
        id,
        title: "p".into(),
        version: None,
        body: None,
        milestone_id: milestone,
        status: ContainerStatus::Open,
        created_at: "t".into(),
        updated_at: updated.into(),
        sort_order: None,
    }
}

fn snap(issues: Vec<Issue>, plans: Vec<(Container, i64)>) -> DashboardSnapshot {
    DashboardSnapshot {
        issues,
        plans,
        milestones: vec![],
        project: "mint".into(),
        milestone_directs: vec![],
    }
}

fn snap_full(
    issues: Vec<Issue>,
    plans: Vec<(Container, i64)>,
    milestones: Vec<(Container, i64)>,
) -> DashboardSnapshot {
    DashboardSnapshot {
        issues,
        plans,
        milestones,
        project: "mint".into(),
        milestone_directs: vec![],
    }
}
