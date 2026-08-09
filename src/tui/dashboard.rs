//! dashboard 状态机：视图切换、变更流、面板自动切换（无 ratatui，可单测）。

use crossterm::event::KeyCode;

use crate::models::{Container, Issue};
use crate::tui::dashboard_diff::{DashboardSnapshot, diff_snapshots};
use crate::tui::dashboard_types::{MAX_FEED, active_plans};

pub use crate::tui::dashboard_types::{FeedItem, RefreshResult, View};

/// dashboard 状态机。
pub struct DashboardModel {
    pub view: View,
    /// 变更流，index 0 = 最新。
    pub feed: Vec<FeedItem>,
    /// 当前面板列表内选中下标。
    pub selected: usize,
    /// 最新快照（详情/进度数据源）。
    pub issues: Vec<Issue>,
    pub plans: Vec<(Container, i64)>,
    prev: Option<DashboardSnapshot>,
    /// 上次自动切到的 plan（用户手动 Esc 回 issue 后避免同 plan 反复抢占）。
    last_auto: Option<i64>,
    /// 展开详情的 issue id（Enter 进入，Esc 收起）。
    pub detail: Option<i64>,
}

impl Default for DashboardModel {
    fn default() -> Self {
        Self::new()
    }
}

impl DashboardModel {
    pub fn new() -> Self {
        Self {
            view: View::Issue,
            feed: Vec::new(),
            selected: 0,
            issues: Vec::new(),
            plans: Vec::new(),
            prev: None,
            last_auto: None,
            detail: None,
        }
    }

    /// 首轮基线：feed = 当前全量按 updated_at 倒序，无事件。
    pub fn init(&mut self, snapshot: DashboardSnapshot) {
        let mut baseline: Vec<FeedItem> = snapshot
            .issues
            .iter()
            .map(|i| FeedItem::Baseline { issue: i.clone() })
            .collect();
        baseline.sort_by(|a, b| {
            b.issue()
                .map(|i| &i.updated_at)
                .cmp(&a.issue().map(|i| &i.updated_at))
        });
        self.feed = baseline;
        let mut issues = snapshot.issues.clone();
        issues.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        self.issues = issues;
        self.plans = snapshot.plans.clone();
        self.prev = Some(snapshot);
        self.view = View::Issue;
        self.selected = 0;
        self.last_auto = None;
    }

    /// 每 tick：diff 上一轮 → 事件前置 feed；面板自动切换。
    pub fn refresh(&mut self, snapshot: &DashboardSnapshot) -> RefreshResult {
        let events = self
            .prev
            .as_ref()
            .map(|p| diff_snapshots(p, snapshot))
            .unwrap_or_default();
        let n = events.len();
        for ev in events.into_iter().rev() {
            self.feed.insert(0, FeedItem::Event(ev));
        }
        if self.feed.len() > MAX_FEED {
            self.feed.truncate(MAX_FEED);
        }
        let auto_plan = self.switch_panel(snapshot);
        let mut issues = snapshot.issues.clone();
        issues.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        self.issues = issues;
        self.plans = snapshot.plans.clone();
        self.prev = Some(snapshot.clone());
        self.clamp_selected();
        // 详情指向的 issue 已删除 → 收起
        if let Some(id) = self.detail
            && !self.issues.iter().any(|i| i.id == id)
        {
            self.detail = None;
        }
        RefreshResult {
            new_events: n,
            auto_plan,
        }
    }

    /// 面板自动切换：issue → 执行中 plan；plan 执行结束 → 回 issue。
    fn switch_panel(&mut self, snap: &DashboardSnapshot) -> Option<i64> {
        let active = active_plans(snap);
        match self.view {
            View::Issue => {
                if let Some(p) = active.first()
                    && self.last_auto != Some(*p)
                {
                    self.view = View::Plan { plan_id: *p };
                    self.last_auto = Some(*p);
                    return Some(*p);
                }
            }
            View::Plan { plan_id } => {
                if !active.contains(&plan_id) {
                    self.view = View::Issue;
                    self.last_auto = None;
                }
            }
        }
        None
    }

    /// 处理按键：返回 true = 退出 dashboard。
    pub fn handle_key(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::Char('j') | KeyCode::Down => {
                let len = self.visible_issues().len();
                if self.selected + 1 < len {
                    self.selected += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
            }
            KeyCode::Enter => {
                if self.detail.is_none()
                    && let Some(i) = self.visible_issues().get(self.selected)
                {
                    self.detail = Some(i.id);
                }
            }
            KeyCode::Esc => {
                if self.detail.is_some() {
                    self.detail = None;
                } else if matches!(self.view, View::Plan { .. }) {
                    // 用户手动返回 issue；保留 last_auto，同 plan 继续执行时不反复抢占
                    self.view = View::Issue;
                    self.clamp_selected();
                }
            }
            KeyCode::Char('q') => return true,
            _ => {}
        }
        false
    }

    /// 当前视图展示的 issue 集合（issue 面板 = 全部；plan 面板 = 该 plan 下）。
    pub fn visible_issues(&self) -> Vec<&Issue> {
        match self.view {
            View::Issue => self.issues.iter().collect(),
            View::Plan { plan_id } => self
                .issues
                .iter()
                .filter(|i| i.plan_id == Some(plan_id))
                .collect(),
        }
    }

    /// 面板切换后校正选中（避免越界）。
    fn clamp_selected(&mut self) {
        let len = self.visible_issues().len();
        if self.selected >= len {
            self.selected = len.saturating_sub(1);
        }
    }

    /// 按 id 查 issue（详情数据源）。
    pub fn issue(&self, id: i64) -> Option<&Issue> {
        self.issues.iter().find(|i| i.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ContainerStatus, Kind, Status};
    use crate::tui::dashboard_diff::DashboardSnapshot;
    use rstest::rstest;

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
            roadmap_id: None,
            status: ContainerStatus::Open,
            created_at: "t".into(),
            updated_at: "t".into(),
        }
    }

    fn snap(issues: Vec<Issue>, plans: Vec<(Container, i64)>) -> DashboardSnapshot {
        DashboardSnapshot { issues, plans }
    }

    #[test]
    fn baseline_sorted_and_selection_kept() {
        let mut m = DashboardModel::new();
        m.init(snap(
            vec![
                mk_issue(1, Status::Open, None, "10:00"),
                mk_issue(2, Status::Dev, None, "12:00"),
            ],
            vec![],
        ));
        assert_eq!(m.view, View::Issue);
        assert_eq!(m.feed[0].issue().unwrap().id, 2); // 最新在前
        let r = m.refresh(&snap(
            vec![
                mk_issue(1, Status::Open, None, "10:00"),
                mk_issue(2, Status::Dev, None, "12:00"),
                mk_issue(3, Status::Open, None, "13:00"),
            ],
            vec![],
        ));
        assert_eq!(r.new_events, 1);
        assert_eq!(m.selected, 0); // 保持
        assert_eq!(m.feed[0].issue().unwrap().id, 3);
    }

    #[test]
    fn selection_clamped_when_list_shrinks() {
        let mut m = DashboardModel::new();
        m.init(snap(vec![mk_issue(1, Status::Open, None, "1")], vec![]));
        m.selected = 5;
        m.refresh(&snap(vec![mk_issue(1, Status::Open, None, "1")], vec![]));
        assert_eq!(m.selected, 0);
    }

    #[test]
    fn panel_switches_on_plan_execution_and_end() {
        let mut m = DashboardModel::new();
        m.init(snap(vec![], vec![]));
        assert_eq!(m.view, View::Issue);
        let r = m.refresh(&snap(
            vec![mk_issue(1, Status::Dev, Some(7), "11:00")],
            vec![(mk_container(7), 1)],
        ));
        assert_eq!(r.auto_plan, Some(7));
        assert_eq!(m.view, View::Plan { plan_id: 7 });
        // 执行结束（全 done）→ 回 issue
        let r = m.refresh(&snap(
            vec![mk_issue(1, Status::Done, Some(7), "12:00")],
            vec![(mk_container(7), 1)],
        ));
        assert_eq!(r.auto_plan, None);
        assert_eq!(m.view, View::Issue);
    }

    #[test]
    fn user_esc_prevents_reclaiming_same_plan() {
        let mut m = DashboardModel::new();
        m.init(snap(vec![], vec![]));
        m.refresh(&snap(
            vec![mk_issue(1, Status::Dev, Some(7), "11:00")],
            vec![(mk_container(7), 1)],
        ));
        assert_eq!(m.view, View::Plan { plan_id: 7 });
        m.handle_key(KeyCode::Esc);
        assert_eq!(m.view, View::Issue);
        let r = m.refresh(&snap(
            vec![mk_issue(1, Status::Dev, Some(7), "11:30")],
            vec![(mk_container(7), 1)],
        ));
        assert_eq!(r.auto_plan, None); // 不抢占
        assert_eq!(m.view, View::Issue);
    }

    #[rstest]
    #[case(KeyCode::Char('j'), 1)]
    #[case(KeyCode::Down, 1)]
    #[case(KeyCode::Char('k'), 0)]
    #[case(KeyCode::Up, 0)]
    fn navigation_keys(#[case] key: KeyCode, #[case] sel: usize) {
        let mut m = DashboardModel::new();
        m.init(snap(
            vec![
                mk_issue(1, Status::Open, None, "1"),
                mk_issue(2, Status::Open, None, "2"),
            ],
            vec![],
        ));
        m.handle_key(key);
        assert_eq!(m.selected, sel);
    }

    #[test]
    fn enter_detail_and_esc_back() {
        let mut m = DashboardModel::new();
        m.init(snap(vec![mk_issue(1, Status::Open, None, "1")], vec![]));
        m.handle_key(KeyCode::Enter);
        assert_eq!(m.detail, Some(1));
        m.handle_key(KeyCode::Esc);
        assert_eq!(m.detail, None);
        assert!(m.handle_key(KeyCode::Char('q')));
    }

    #[test]
    fn visible_issues_filters_by_plan() {
        let mut m = DashboardModel::new();
        m.init(snap(
            vec![
                mk_issue(1, Status::Dev, Some(7), "1"),
                mk_issue(2, Status::Open, None, "2"),
            ],
            vec![],
        ));
        assert_eq!(m.visible_issues().len(), 2);
        m.view = View::Plan { plan_id: 7 };
        let v = m.visible_issues();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].id, 1);
    }
}
