//! dashboard 状态机：视图切换、变更流、面板自动切换（无 ratatui，可单测）。

use crossterm::event::KeyCode;

use crate::models::{Container, Issue, Status};
use crate::tui::dashboard_diff::{ChangeEvent, DashboardSnapshot, diff_snapshots};

/// 变更流最大条目数（超出裁剪尾部）。
pub const MAX_FEED: usize = 200;

/// 视图：默认 issue 面板；plan 执行中（有 dev/test issue）自动切 plan 面板。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Issue,
    Plan { plan_id: i64 },
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

/// dashboard 状态机。
pub struct DashboardModel {
    pub view: View,
    /// 变更流，index 0 = 最新。
    pub feed: Vec<FeedItem>,
    /// feed 内选中下标。
    pub selected: usize,
    /// 最新快照（详情/进度数据源）。
    pub issues: Vec<Issue>,
    pub plans: Vec<(Container, i64)>,
    prev: Option<DashboardSnapshot>,
    /// 上次自动切到的 plan（用户手动 Esc 回 issue 后避免同 plan 反复抢占）。
    last_auto: Option<i64>,
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
            KeyCode::Esc => {
                if matches!(self.view, View::Plan { .. }) {
                    // 用户手动返回 issue；保留 last_auto，同 plan 继续执行时不反复抢占
                    self.view = View::Issue;
                }
                self.clamp_selected();
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
    use crate::models::{ContainerStatus, Kind};
    use crate::tui::dashboard_diff::DashboardSnapshot;
    use rstest::rstest;

    fn mk_issue(
        id: i64,
        title: &str,
        status: Status,
        plan_id: Option<i64>,
        updated: &str,
    ) -> Issue {
        Issue {
            id,
            title: title.into(),
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

    fn mk_container(id: i64, title: &str) -> Container {
        Container {
            id,
            title: title.into(),
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
    fn init_baseline_sorted_by_updated_desc() {
        let mut m = DashboardModel::new();
        m.init(snap(
            vec![
                mk_issue(1, "old", Status::Open, None, "10:00"),
                mk_issue(2, "new", Status::Dev, None, "12:00"),
            ],
            vec![],
        ));
        assert_eq!(m.view, View::Issue);
        assert_eq!(m.feed.len(), 2);
        assert_eq!(m.feed[0].issue().unwrap().id, 2); // 最新在前
        assert_eq!(m.feed[1].issue().unwrap().id, 1);
    }

    #[test]
    fn refresh_prepends_events_and_clamps_selection() {
        let mut m = DashboardModel::new();
        m.init(snap(
            vec![mk_issue(1, "a", Status::Open, None, "10:00")],
            vec![],
        ));
        // 新增 issue 事件 → feed 前置，selected 保持（面板列表下标）
        let r = m.refresh(&snap(
            vec![
                mk_issue(1, "a", Status::Open, None, "10:00"),
                mk_issue(2, "b", Status::Open, None, "11:00"),
            ],
            vec![],
        ));
        assert_eq!(r.new_events, 1);
        assert_eq!(m.selected, 0);
        assert_eq!(m.feed[0].issue().unwrap().id, 2);
        // 状态变化事件，issues 数量不变 → selected 保持
        m.selected = 1;
        let r = m.refresh(&snap(
            vec![
                mk_issue(1, "a", Status::Open, None, "10:00"),
                mk_issue(2, "b", Status::Dev, None, "11:30"),
            ],
            vec![],
        ));
        assert_eq!(r.new_events, 1);
        assert_eq!(m.selected, 1);
        // selected 越界 → clamp
        m.selected = 5;
        m.refresh(&snap(
            vec![mk_issue(1, "a", Status::Open, None, "10:00")],
            vec![],
        ));
        assert_eq!(m.selected, 0);
    }

    #[test]
    fn panel_switches_on_plan_execution() {
        let mut m = DashboardModel::new();
        m.init(snap(vec![], vec![]));
        // 无执行 → issue
        assert_eq!(m.view, View::Issue);
        // plan 有 dev issue → 自动切 plan
        let r = m.refresh(&snap(
            vec![mk_issue(1, "dev", Status::Dev, Some(7), "11:00")],
            vec![(mk_container(7, "sprint"), 1)],
        ));
        assert_eq!(r.auto_plan, Some(7));
        assert_eq!(m.view, View::Plan { plan_id: 7 });
        // 执行结束（全 done）→ 回 issue
        let r = m.refresh(&snap(
            vec![mk_issue(1, "done", Status::Done, Some(7), "12:00")],
            vec![(mk_container(7, "sprint"), 1)],
        ));
        assert_eq!(r.auto_plan, None);
        assert_eq!(m.view, View::Issue);
    }

    #[test]
    fn user_esc_prevents_reclaiming_same_plan() {
        let mut m = DashboardModel::new();
        m.init(snap(vec![], vec![]));
        m.refresh(&snap(
            vec![mk_issue(1, "dev", Status::Dev, Some(7), "11:00")],
            vec![(mk_container(7, "sprint"), 1)],
        ));
        assert_eq!(m.view, View::Plan { plan_id: 7 });
        // 用户 Esc 回 issue
        m.handle_key(KeyCode::Esc);
        assert_eq!(m.view, View::Issue);
        // 同 plan 仍执行 → 不抢占（last_auto == 7）
        let r = m.refresh(&snap(
            vec![mk_issue(1, "dev", Status::Dev, Some(7), "11:30")],
            vec![(mk_container(7, "sprint"), 1)],
        ));
        assert_eq!(r.auto_plan, None);
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
                mk_issue(1, "a", Status::Open, None, "1"),
                mk_issue(2, "b", Status::Open, None, "2"),
            ],
            vec![],
        ));
        m.handle_key(key);
        assert_eq!(m.selected, sel);
    }

    #[test]
    fn quit_key() {
        let mut m = DashboardModel::new();
        m.init(snap(vec![], vec![]));
        assert!(m.handle_key(KeyCode::Char('q')));
        assert!(!m.handle_key(KeyCode::Enter));
    }

    #[test]
    fn visible_issues_filters_by_plan() {
        let mut m = DashboardModel::new();
        m.init(snap(
            vec![
                mk_issue(1, "in", Status::Dev, Some(7), "1"),
                mk_issue(2, "out", Status::Open, None, "2"),
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
