//! dashboard 公共类型：视图、变更流条目、刷新结果、执行中 plan 判定。

use crate::models::{Issue, Status};
use crate::tui::dashboard::diff::{ChangeEvent, DashboardSnapshot};

/// 变更流最大条目数（超出裁剪尾部）。
pub const MAX_FEED: usize = 200;

/// 视图：默认 issue 面板；plan 执行中（有 dev/test issue）自动切 plan 面板；
/// milestone 面板显示其下 plan 分组（手动 Tab/p 循环进入，或 plan 结束时短暂自动切入）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Issue,
    Plan { plan_id: i64 },
    Milestone { milestone_id: i64 },
}

/// 变更流条目：初始基线（当前全量按 updated_at 倒序）或会话内变化事件。
#[derive(Debug, Clone)]
pub enum FeedItem {
    Baseline { issue: Issue },
    Event(ChangeEvent),
}

impl FeedItem {
    /// 条目关联的 issue（plan 事件返回 None）。
    pub fn issue(&self) -> Option<&Issue> {
        match self {
            FeedItem::Baseline { issue } => Some(issue),
            FeedItem::Event(
                ChangeEvent::IssueAdded { issue }
                | ChangeEvent::IssueStatusChanged { issue, .. }
                | ChangeEvent::IssueUpdated { issue },
            ) => Some(issue),
            _ => None,
        }
    }
}

/// 一次 refresh 的结果。
pub struct RefreshResult {
    pub new_events: usize,
    pub auto_plan: Option<i64>,
}

/// 执行中 plan：有 dev/test issue 的 plan，按该 plan 下最新活跃 issue updated_at 降序。
pub fn active_plans(snap: &DashboardSnapshot) -> Vec<i64> {
    let mut by_plan: Vec<(i64, &str)> = snap
        .issues
        .iter()
        .filter(|i| matches!(i.status, Status::Dev | Status::Test))
        .filter_map(|i| i.plan_id.map(|pid| (pid, i.updated_at.as_str())))
        .collect();
    by_plan.sort_by(|a, b| b.1.cmp(a.1));
    let mut seen = std::collections::HashSet::new();
    by_plan
        .into_iter()
        .filter_map(|(pid, _)| seen.insert(pid).then_some(pid))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Container, ContainerStatus, Kind};

    fn mk_issue(id: i64, status: Status, plan_id: Option<i64>, updated: &str) -> Issue {
        Issue {
            id,
            title: "t".into(),
            body: None,
            kind: Kind::Problem,
            status,
            priority: 3,
            project_id: 1,
            project: Some("mint".into()),
            test_cmd: None,
            dropped_reason: None,
            last_commit_id: None,
            plan_id,
            hit_count: 0,
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
        }
    }

    #[test]
    fn active_plans_filters_dev_and_sorts_by_recent() {
        let snap = DashboardSnapshot {
            issues: vec![
                mk_issue(1, Status::Dev, Some(7), "10:00"),
                mk_issue(2, Status::Open, Some(7), "12:00"),
                mk_issue(3, Status::Test, Some(8), "11:00"),
                mk_issue(4, Status::Dev, Some(8), "09:00"),
                mk_issue(5, Status::Done, Some(8), "13:00"),
            ],
            plans: vec![(mk_container(7), 2), (mk_container(8), 3)],
            milestones: vec![],
        };
        // 活跃 issue 最新更新：plan 7 的 dev(10:00) vs plan 8 的 test(11:00) → 8 在前
        let active = active_plans(&snap);
        assert_eq!(active, vec![8, 7]);
    }

    #[test]
    fn feed_item_issue_only_for_issue_events() {
        use crate::tui::dashboard::diff::ChangeEvent;
        let i = mk_issue(1, Status::Open, None, "1");
        assert_eq!(
            FeedItem::Baseline { issue: i.clone() }
                .issue()
                .map(|x| x.id),
            Some(1)
        );
        assert_eq!(
            FeedItem::Event(ChangeEvent::IssueAdded { issue: i.clone() })
                .issue()
                .map(|x| x.id),
            Some(1)
        );
        assert!(
            FeedItem::Event(ChangeEvent::PlanAdded {
                plan: mk_container(9),
                count: 0
            })
            .issue()
            .is_none()
        );
    }
}
