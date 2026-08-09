//! 空闲回首页（规则 7）：无操作、无跳转 60s → 直接跳首页（不经 queue）。

use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::types::{HOME_TIMEOUT, View};

impl DashboardModel {
    /// 规则 7：用户空闲 ≥ HOME_TIMEOUT 且无待跳请求且不在首页 → 回首页（重置空闲，避免反复）。
    pub(crate) fn home_timeout(&mut self) {
        if self.user_idle < HOME_TIMEOUT {
            return;
        }
        if !self.pending.is_empty() || !self.ready.is_empty() {
            return;
        }
        if self.view == View::Issues {
            return;
        }
        self.view = View::Issues;
        self.page = 0;
        self.selected = 0;
        self.user_idle = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Issue, Kind, Status};
    use crate::tui::dashboard::diff::DashboardSnapshot;

    fn snap(issues: Vec<Issue>) -> DashboardSnapshot {
        DashboardSnapshot {
            issues,
            plans: vec![],
            milestones: vec![],
            project: "mint".into(),
        }
    }

    fn mk_issue(id: i64) -> Issue {
        Issue {
            id,
            title: "t".into(),
            body: None,
            kind: Kind::Problem,
            status: Status::Open,
            priority: 3,
            project_id: 1,
            project: Some("mint".into()),
            test_cmd: None,
            dropped_reason: None,
            last_commit_id: None,
            plan_id: None,
            hit_count: 0,
            labels: vec![],
            links: vec![],
            created_at: "t".into(),
            updated_at: "t".into(),
        }
    }

    #[test]
    fn home_timeout_after_idle() {
        let mut m = DashboardModel::new();
        m.init(snap(vec![mk_issue(1)]));
        m.view = View::Plans;
        m.user_idle = HOME_TIMEOUT - 1;
        m.home_timeout();
        assert_eq!(m.view, View::Plans); // 未到 60s 不跳
        m.user_idle = HOME_TIMEOUT;
        m.home_timeout();
        assert_eq!(m.view, View::Issues);
    }

    #[test]
    fn home_timeout_skipped_when_jumps_pending() {
        let mut m = DashboardModel::new();
        m.init(snap(vec![]));
        m.view = View::Plans;
        m.user_idle = HOME_TIMEOUT;
        m.pending.push_back(crate::tui::dashboard::types::RawJump {
            target: crate::tui::dashboard::types::JumpTarget::Plans,
        });
        m.home_timeout();
        assert_eq!(m.view, View::Plans); // 有待跳请求不跳
    }

    #[test]
    fn home_timeout_noop_on_home() {
        let mut m = DashboardModel::new();
        m.init(snap(vec![]));
        m.view = View::Issues;
        m.user_idle = HOME_TIMEOUT;
        m.home_timeout();
        assert_eq!(m.view, View::Issues);
    }

    #[test]
    fn home_timeout_resets_idle() {
        let mut m = DashboardModel::new();
        m.init(snap(vec![]));
        m.view = View::Milestones;
        m.user_idle = HOME_TIMEOUT;
        m.home_timeout();
        assert_eq!(m.user_idle, 0);
    }
}
