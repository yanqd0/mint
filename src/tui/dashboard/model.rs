//! dashboard 状态机：视图切换、变更流、面板自动切换（无 ratatui，可单测）。
//! 从 dashboard.rs 拆分而来；入口聚合导出见 `dashboard`。

use crossterm::event::KeyCode;

use crate::models::{Container, Issue, Status};
use crate::tui::dashboard::diff::{DashboardSnapshot, diff_snapshots};
use crate::tui::dashboard::types::{MAX_FEED, active_plans};

pub use crate::tui::dashboard::types::{FeedItem, RefreshResult, View};

/// plan 执行结束 → 所属 milestone 详情的自动停留 tick 数（1 tick = 1s）。
const MILESTONE_HOLD_TICKS: u32 = 3;
/// 自动切换前置：用户空闲至少这么多 tick（1 tick = 1s）才允许自动切换。
const AUTO_SWITCH_IDLE: u32 = 5;
/// 两次自动切换的最小间隔（tick）。
const AUTO_SWITCH_GAP: u32 = 5;

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
    pub milestones: Vec<(Container, i64)>,
    prev: Option<DashboardSnapshot>,
    /// 上次自动切到的 plan（用户手动 Esc 回 tab 后避免同 plan 反复抢占）。
    last_auto: Option<i64>,
    /// 当前面板页（0-based，每页 page_size 行）。
    pub page: usize,
    /// 每页行数。
    pub page_size: usize,
    /// 用户空闲 tick（handle_key 重置 0，refresh 递增）；自动切换前置 ≥ AUTO_SWITCH_IDLE。
    user_idle: u32,
    /// 距上次自动切换的 tick；两次自动切换间隔 ≥ AUTO_SWITCH_GAP。
    auto_last: u32,
    /// 自动临时进入 milestone 详情的剩余 tick（Some = 自动切入，None = 用户手动/未切入）。
    milestone_hold: Option<u32>,
}

/// Plans 页分组：一组 panel 的数据（组标题 + plan 行引用）。
pub struct PlanGroup<'a> {
    pub title: String,
    pub plans: Vec<&'a (Container, i64)>,
}

impl Default for DashboardModel {
    fn default() -> Self {
        Self::new()
    }
}

impl DashboardModel {
    pub fn new() -> Self {
        Self {
            view: View::Issues,
            feed: Vec::new(),
            selected: 0,
            issues: Vec::new(),
            plans: Vec::new(),
            milestones: Vec::new(),
            prev: None,
            last_auto: None,
            page: 0,
            page_size: 10,
            user_idle: 0,
            auto_last: 0,
            milestone_hold: None,
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
        self.milestones = snapshot.milestones.clone();
        self.prev = Some(snapshot);
        self.view = View::Issues;
        self.selected = 0;
        self.last_auto = None;
        self.user_idle = 0;
        self.auto_last = 0;
        self.milestone_hold = None;
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
        // tick 计数：空闲与自动切换间隔递增。
        self.user_idle = self.user_idle.saturating_add(1);
        self.auto_last = self.auto_last.saturating_add(1);
        // 自动临时 milestone 详情倒计时递减（归零后 switch_panel 回 Plans tab）。
        if let Some(hold) = self.milestone_hold.as_mut() {
            *hold = hold.saturating_sub(1);
        }
        let auto_plan = self.switch_panel(snapshot);
        let mut issues = snapshot.issues.clone();
        issues.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        self.issues = issues;
        self.plans = snapshot.plans.clone();
        self.milestones = snapshot.milestones.clone();
        self.prev = Some(snapshot.clone());
        self.clamp_selected();
        self.clamp_page();
        // 详情指向的实体已删除 → 回对应 tab。
        self.prune_detail();
        RefreshResult {
            new_events: n,
            auto_plan,
        }
    }

    /// 详情指向的实体已不存在 → 回对应 tab。
    fn prune_detail(&mut self) {
        let back = match self.view {
            View::IssueDetail { id } if !self.issues.iter().any(|i| i.id == id) => {
                Some(View::Issues)
            }
            View::PlanDetail { plan_id } if !self.plans.iter().any(|(c, _)| c.id == plan_id) => {
                Some(View::Plans)
            }
            View::MilestoneDetail { milestone_id }
                if !self.milestones.iter().any(|(c, _)| c.id == milestone_id) =>
            {
                Some(View::Milestones)
            }
            _ => None,
        };
        if let Some(v) = back {
            self.view = v;
            self.page = 0;
            self.clamp_selected();
        }
    }

    /// 面板自动切换（空闲/间隔前置）：tab → 执行中 plan 详情；plan 结束 → 所属 milestone 详情 → 回 Plans tab。
    fn switch_panel(&mut self, snap: &DashboardSnapshot) -> Option<i64> {
        // 用户操作后 ≥5s 且距上次自动切换 ≥5s 才允许自动切换。
        if self.user_idle < AUTO_SWITCH_IDLE || self.auto_last < AUTO_SWITCH_GAP {
            return None;
        }
        let active = active_plans(snap);
        match self.view {
            View::Issues | View::Plans | View::Milestones => {
                if let Some(p) = active.first()
                    && self.last_auto != Some(*p)
                {
                    self.view = View::PlanDetail { plan_id: *p };
                    self.last_auto = Some(*p);
                    self.auto_last = 0;
                    self.page = 0;
                    self.selected = 0;
                    return Some(*p);
                }
            }
            View::PlanDetail { plan_id } => {
                if !active.contains(&plan_id) {
                    // plan 执行结束：若属某 milestone，短暂切到其 milestone 详情供扫一眼再回 Plans tab
                    if let Some((plan, _)) = snap.plans.iter().find(|(c, _)| c.id == plan_id)
                        && let Some(mid) = plan.milestone_id
                    {
                        self.view = View::MilestoneDetail { milestone_id: mid };
                        self.milestone_hold = Some(MILESTONE_HOLD_TICKS);
                    } else {
                        self.view = View::Plans;
                    }
                    self.last_auto = None;
                    self.auto_last = 0;
                    self.page = 0;
                    self.selected = 0;
                }
            }
            View::MilestoneDetail { .. } => {
                // 自动临时切入的 milestone 详情倒计时归零 → 回 Plans tab。
                if self.milestone_hold == Some(0) {
                    self.view = View::Plans;
                    self.milestone_hold = None;
                    self.auto_last = 0;
                    self.page = 0;
                    self.selected = 0;
                }
            }
            View::IssueDetail { .. } => {}
        }
        None
    }

    /// 处理按键：返回 true = 退出 dashboard。
    pub fn handle_key(&mut self, key: KeyCode) -> bool {
        // 任何按键 → 用户活跃，重置空闲计时（自动切换前置失效）。
        self.user_idle = 0;
        // 用户手动操作 milestone 详情（Esc/q 之外）→ 接管，取消自动倒计时踢回。
        if matches!(self.view, View::MilestoneDetail { .. })
            && self.milestone_hold.is_some()
            && !matches!(key, KeyCode::Esc | KeyCode::Char('q'))
        {
            self.milestone_hold = None;
        }
        match key {
            KeyCode::Char('1') => self.switch_tab(View::Issues),
            KeyCode::Char('2') => self.switch_tab(View::Plans),
            KeyCode::Char('3') => self.switch_tab(View::Milestones),
            KeyCode::Tab => {
                let next = match self.active_tab() {
                    View::Issues => View::Plans,
                    View::Plans => View::Milestones,
                    _ => View::Issues,
                };
                self.switch_tab(next);
            }
            KeyCode::Char('j') | KeyCode::Down => {
                let len = self.current_page_len();
                if self.selected + 1 < len {
                    self.selected += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
            }
            KeyCode::Char('h') | KeyCode::Left | KeyCode::PageUp => {
                if self.page > 0 {
                    self.page -= 1;
                    self.selected = 0;
                }
            }
            KeyCode::Char('l') | KeyCode::Right | KeyCode::PageDown => {
                if self.page + 1 < self.pages() {
                    self.page += 1;
                    self.selected = 0;
                }
            }
            KeyCode::Char('p') => {
                if let Some(pid) = self.selected_plan_id() {
                    self.view = View::PlanDetail { plan_id: pid };
                    self.page = 0;
                    self.selected = 0;
                }
            }
            KeyCode::Char('r') => {
                if let Some(mid) = self.selected_milestone_id() {
                    self.view = View::MilestoneDetail { milestone_id: mid };
                    self.page = 0;
                    self.selected = 0;
                }
            }
            KeyCode::Enter => match self.view {
                View::Issues => {
                    if let Some(i) = self.page_issues().get(self.selected) {
                        self.view = View::IssueDetail { id: i.id };
                        self.page = 0;
                        self.selected = 0;
                    }
                }
                View::Plans => {
                    if let Some((c, _)) = self.page_plans().get(self.selected) {
                        self.view = View::PlanDetail { plan_id: c.id };
                        self.page = 0;
                        self.selected = 0;
                    }
                }
                View::Milestones => {
                    if let Some((c, _)) = self.page_milestones().get(self.selected) {
                        self.view = View::MilestoneDetail { milestone_id: c.id };
                        self.page = 0;
                        self.selected = 0;
                    }
                }
                View::MilestoneDetail { .. } => {
                    // milestone 详情：进入选中的 plan 详情。
                    if let Some((c, _)) = self.page_plans().get(self.selected) {
                        self.view = View::PlanDetail { plan_id: c.id };
                        self.page = 0;
                        self.selected = 0;
                    }
                }
                _ => {}
            },
            KeyCode::Esc => match self.view {
                View::IssueDetail { .. } => self.switch_tab(View::Issues),
                View::PlanDetail { .. } => self.switch_tab(View::Plans),
                View::MilestoneDetail { .. } => {
                    self.milestone_hold = None;
                    self.switch_tab(View::Milestones);
                }
                _ => {}
            },
            KeyCode::Char('q') => return true,
            _ => {}
        }
        false
    }

    /// 切到 tab 页（清空行状态）。
    fn switch_tab(&mut self, tab: View) {
        self.view = tab;
        self.page = 0;
        self.selected = 0;
    }

    /// 当前所属 tab（详情页归其 tab）。
    fn active_tab(&self) -> View {
        match self.view {
            View::Issues | View::IssueDetail { .. } => View::Issues,
            View::Plans | View::PlanDetail { .. } => View::Plans,
            View::Milestones | View::MilestoneDetail { .. } => View::Milestones,
        }
    }

    /// 选中行的 plan id（Issues 行的 plan_id 或 Plans 行的 plan id）。
    fn selected_plan_id(&self) -> Option<i64> {
        match self.view {
            View::Issues => self
                .page_issues()
                .get(self.selected)
                .and_then(|i| i.plan_id),
            View::Plans => self.page_plans().get(self.selected).map(|(c, _)| c.id),
            _ => None,
        }
    }

    /// 选中行的 milestone id（issue 所属 plan 的 milestone / plan 的 milestone / milestone 行）。
    fn selected_milestone_id(&self) -> Option<i64> {
        match self.view {
            View::Issues => self
                .page_issues()
                .get(self.selected)
                .and_then(|i| i.plan_id)
                .and_then(|pid| self.plans.iter().find(|(c, _)| c.id == pid))
                .and_then(|(c, _)| c.milestone_id),
            View::Plans => self
                .page_plans()
                .get(self.selected)
                .and_then(|(c, _)| c.milestone_id),
            View::Milestones => self.page_milestones().get(self.selected).map(|(c, _)| c.id),
            _ => None,
        }
    }

    /// 当前视图展示的 issue 集合（Issues tab = 全部；PlanDetail = 该 plan 下；
    /// MilestoneDetail = 其下 plan 的 issue 聚合）。
    pub fn visible_issues(&self) -> Vec<&Issue> {
        match self.view {
            View::Issues => self.issues.iter().collect(),
            View::PlanDetail { plan_id } => self
                .issues
                .iter()
                .filter(|i| i.plan_id == Some(plan_id))
                .collect(),
            View::MilestoneDetail { milestone_id } => self
                .issues
                .iter()
                .filter(|i| {
                    i.plan_id
                        .and_then(|pid| self.plans.iter().find(|(c, _)| c.id == pid))
                        .map(|(c, _)| c.milestone_id == Some(milestone_id))
                        .unwrap_or(false)
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Plans tab 行：分组展平（执行中 milestone → 无 milestone → 剩余 milestone）。
    pub fn visible_plans(&self) -> Vec<&(Container, i64)> {
        let mut out = Vec::new();
        for g in self.plan_groups() {
            out.extend(g.plans);
        }
        out
    }

    /// Plans 页分组：执行中 milestone（活跃 plan 所属）→ 无 milestone 的 plan → 剩余 milestone 按 updated_at 逆序。
    pub fn plan_groups(&self) -> Vec<PlanGroup<'_>> {
        let mut active: Vec<i64> = self
            .issues
            .iter()
            .filter(|i| matches!(i.status, Status::Dev | Status::Test))
            .filter_map(|i| i.plan_id)
            .collect();
        active.sort_unstable();
        active.dedup();

        let plan_milestone = |pid: i64| -> Option<i64> {
            self.plans
                .iter()
                .find(|(c, _)| c.id == pid)
                .and_then(|(c, _)| c.milestone_id)
        };

        // 1. 执行中的 milestone（活跃 plan 所属）。
        let mut active_ms: Vec<i64> = active.iter().filter_map(|&p| plan_milestone(p)).collect();
        active_ms.sort_unstable();
        active_ms.dedup();
        let mut groups: Vec<PlanGroup> = Vec::new();
        for &mid in &active_ms {
            let title = self.milestone_title(mid);
            let plans = self.milestone_plans(mid);
            groups.push(PlanGroup { title, plans });
        }

        // 2. 无 milestone（或 milestone 已不存在）的 plan。
        let free: Vec<&(Container, i64)> = self
            .plans
            .iter()
            .filter(|(c, _)| match c.milestone_id {
                None => true,
                Some(mid) => !self.milestones.iter().any(|(ms, _)| ms.id == mid),
            })
            .collect();
        if !free.is_empty() {
            groups.push(PlanGroup {
                title: "no milestone".into(),
                plans: free,
            });
        }

        // 3. 剩余 milestone（非活跃）按 updated_at 逆序。
        let mut rest: Vec<&(Container, i64)> = self
            .milestones
            .iter()
            .filter(|(c, _)| !active_ms.contains(&c.id))
            .collect();
        rest.sort_by(|a, b| b.0.updated_at.cmp(&a.0.updated_at));
        for (ms, _) in rest {
            let title = self.milestone_title(ms.id);
            let plans = self.milestone_plans(ms.id);
            groups.push(PlanGroup { title, plans });
        }
        groups
    }

    /// milestone 标题（含 version，如 `TUI (0.4.0)`）。
    fn milestone_title(&self, id: i64) -> String {
        self.milestones
            .iter()
            .find(|(c, _)| c.id == id)
            .map(|(c, _)| match &c.version {
                Some(v) => format!("{} ({v})", c.title),
                None => c.title.clone(),
            })
            .unwrap_or_else(|| format!("#{id}"))
    }

    /// 某 milestone 下的 plan（MilestoneDetail 用，按 updated_at 逆序）。
    pub fn milestone_plans(&self, milestone_id: i64) -> Vec<&(Container, i64)> {
        let mut ps: Vec<&(Container, i64)> = self
            .plans
            .iter()
            .filter(|(c, _)| c.milestone_id == Some(milestone_id))
            .collect();
        ps.sort_by(|a, b| b.0.updated_at.cmp(&a.0.updated_at));
        ps
    }

    /// Milestones tab 行：全部 milestone（按 updated_at 逆序）。
    pub fn visible_milestones(&self) -> Vec<&(Container, i64)> {
        let mut ms: Vec<&(Container, i64)> = self.milestones.iter().collect();
        ms.sort_by(|a, b| b.0.updated_at.cmp(&a.0.updated_at));
        ms
    }

    /// 某 plan 的完成进度（done / total issue 数）。
    pub fn plan_progress(&self, plan_id: i64) -> (usize, usize) {
        let mut total = 0;
        let mut done = 0;
        for i in &self.issues {
            if i.plan_id == Some(plan_id) {
                total += 1;
                if i.status == Status::Done {
                    done += 1;
                }
            }
        }
        (done, total)
    }

    /// 面板切换后校正选中（避免越界）。
    fn clamp_selected(&mut self) {
        let len = self.current_page_len();
        if self.selected >= len {
            self.selected = len.saturating_sub(1);
        }
    }

    /// 当前面板页内的 issue 集合（列表渲染用）。
    pub fn page_issues(&self) -> Vec<&Issue> {
        let all = self.visible_issues();
        let start = self.page * self.page_size;
        if start >= all.len() {
            return Vec::new();
        }
        let end = (start + self.page_size).min(all.len());
        all[start..end].to_vec()
    }

    /// 当前面板总页数（至少 1，按视图行集合计算）。
    pub fn pages(&self) -> usize {
        let len = match self.view {
            View::Issues | View::PlanDetail { .. } | View::MilestoneDetail { .. } => {
                self.visible_issues().len()
            }
            View::Plans => self.visible_plans().len(),
            View::Milestones => self.visible_milestones().len(),
            View::IssueDetail { .. } => 0,
        };
        len.div_ceil(self.page_size).max(1)
    }

    /// 当前页的 plan 行（Plans tab = 全部；MilestoneDetail = 该 milestone 下）。
    pub fn page_plans(&self) -> Vec<&(Container, i64)> {
        let all = match self.view {
            View::Plans => self.visible_plans(),
            View::MilestoneDetail { milestone_id } => self.milestone_plans(milestone_id),
            _ => return Vec::new(),
        };
        let start = self.page * self.page_size;
        if start >= all.len() {
            return Vec::new();
        }
        let end = (start + self.page_size).min(all.len());
        all[start..end].to_vec()
    }

    /// 当前页的 milestone 行（Milestones tab）。
    pub fn page_milestones(&self) -> Vec<&(Container, i64)> {
        if !matches!(self.view, View::Milestones) {
            return Vec::new();
        }
        let all = self.visible_milestones();
        let start = self.page * self.page_size;
        if start >= all.len() {
            return Vec::new();
        }
        let end = (start + self.page_size).min(all.len());
        all[start..end].to_vec()
    }

    /// 当前面板一页的行数（随视图切换）。
    fn current_page_len(&self) -> usize {
        match self.view {
            View::Issues | View::PlanDetail { .. } | View::MilestoneDetail { .. } => {
                self.page_issues().len()
            }
            View::Plans => self.page_plans().len(),
            View::Milestones => self.page_milestones().len(),
            View::IssueDetail { .. } => 0,
        }
    }

    /// 面板数据变化后校正页号（避免越界）。
    fn clamp_page(&mut self) {
        if self.page >= self.pages() {
            self.page = self.pages().saturating_sub(1);
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
    use crate::tui::dashboard::diff::DashboardSnapshot;
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
            milestone_id: None,
            status: ContainerStatus::Open,
            created_at: "t".into(),
            updated_at: "t".into(),
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
        }
    }

    fn snap(issues: Vec<Issue>, plans: Vec<(Container, i64)>) -> DashboardSnapshot {
        DashboardSnapshot {
            issues,
            plans,
            milestones: vec![],
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
        }
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
        assert_eq!(m.view, View::Issues);
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

    /// 让自动切换前置满足：用户空闲 ≥5s 且距上次自动切换 ≥5s。
    fn enable_auto(m: &mut DashboardModel) {
        m.user_idle = 5;
        m.auto_last = 5;
    }

    #[test]
    fn auto_switch_requires_idle_gap() {
        let mut m = DashboardModel::new();
        m.init(snap(vec![], vec![]));
        // 用户空闲不足（刚 init）→ 有 dev issue 也不自动切
        let r = m.refresh(&snap(
            vec![mk_issue(1, Status::Dev, Some(7), "11:00")],
            vec![(mk_container(7), 1)],
        ));
        assert_eq!(r.auto_plan, None);
        assert_eq!(m.view, View::Issues);
    }

    #[test]
    fn auto_switches_to_plan_detail_and_back_to_plans() {
        let mut m = DashboardModel::new();
        m.init(snap(vec![], vec![]));
        assert_eq!(m.view, View::Issues);
        enable_auto(&mut m);
        let r = m.refresh(&snap(
            vec![mk_issue(1, Status::Dev, Some(7), "11:00")],
            vec![(mk_container(7), 1)],
        ));
        assert_eq!(r.auto_plan, Some(7));
        assert_eq!(m.view, View::PlanDetail { plan_id: 7 });
        // 执行结束（全 done）→ 间隔 ≥5 后回 Plans tab
        let end = snap(
            vec![mk_issue(1, Status::Done, Some(7), "12:00")],
            vec![(mk_container(7), 1)],
        );
        for _ in 0..5 {
            m.refresh(&end);
        }
        assert_eq!(m.view, View::Plans);
    }

    #[test]
    fn user_esc_prevents_reclaiming_same_plan() {
        let mut m = DashboardModel::new();
        m.init(snap(vec![], vec![]));
        enable_auto(&mut m);
        m.refresh(&snap(
            vec![mk_issue(1, Status::Dev, Some(7), "11:00")],
            vec![(mk_container(7), 1)],
        ));
        assert_eq!(m.view, View::PlanDetail { plan_id: 7 });
        m.handle_key(KeyCode::Esc);
        assert_eq!(m.view, View::Plans);
        enable_auto(&mut m);
        let r = m.refresh(&snap(
            vec![mk_issue(1, Status::Dev, Some(7), "11:30")],
            vec![(mk_container(7), 1)],
        ));
        assert_eq!(r.auto_plan, None); // last_auto=7 → 不抢占
        assert_eq!(m.view, View::Plans);
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
        assert_eq!(m.view, View::IssueDetail { id: 1 });
        m.handle_key(KeyCode::Esc);
        assert_eq!(m.view, View::Issues);
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
        m.view = View::PlanDetail { plan_id: 7 };
        let v = m.visible_issues();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].id, 1);
    }

    #[test]
    fn number_keys_and_tab_switch_tabs() {
        let mut m = DashboardModel::new();
        m.init(snap(vec![], vec![]));
        m.handle_key(KeyCode::Char('2'));
        assert_eq!(m.view, View::Plans);
        m.handle_key(KeyCode::Char('3'));
        assert_eq!(m.view, View::Milestones);
        m.handle_key(KeyCode::Char('1'));
        assert_eq!(m.view, View::Issues);
        m.handle_key(KeyCode::Tab);
        assert_eq!(m.view, View::Plans);
        m.handle_key(KeyCode::Tab);
        assert_eq!(m.view, View::Milestones);
        m.handle_key(KeyCode::Tab);
        assert_eq!(m.view, View::Issues);
    }

    #[test]
    fn plans_tab_enter_opens_plan_detail() {
        let mut m = DashboardModel::new();
        m.init(snap(vec![], vec![(mk_plan(7, Some(4), "1"), 0)]));
        m.handle_key(KeyCode::Char('2'));
        m.handle_key(KeyCode::Enter);
        assert_eq!(m.view, View::PlanDetail { plan_id: 7 });
        m.handle_key(KeyCode::Esc);
        assert_eq!(m.view, View::Plans);
    }

    #[test]
    fn p_key_jumps_to_plan_detail_from_issue() {
        let mut m = DashboardModel::new();
        m.init(snap(
            vec![mk_issue(1, Status::Dev, Some(7), "1")],
            vec![(mk_plan(7, Some(4), "1"), 0)],
        ));
        m.handle_key(KeyCode::Char('p'));
        assert_eq!(m.view, View::PlanDetail { plan_id: 7 });
    }

    #[test]
    fn milestone_plans_filters_and_sorts() {
        let mut m = DashboardModel::new();
        m.init(snap(
            vec![],
            vec![
                (mk_plan(7, Some(4), "10:00"), 0),
                (mk_plan(8, Some(4), "12:00"), 0),
                (mk_plan(9, None, "11:00"), 0),
            ],
        ));
        let ps = m.milestone_plans(4);
        assert_eq!(ps.len(), 2);
        assert_eq!(ps[0].0.id, 8); // updated_at 最新在前
        assert_eq!(ps[1].0.id, 7);
    }

    #[test]
    fn plan_progress_counts_done_over_total() {
        let mut m = DashboardModel::new();
        m.init(snap(
            vec![
                mk_issue(1, Status::Done, Some(7), "1"),
                mk_issue(2, Status::Open, Some(7), "2"),
                mk_issue(3, Status::Dev, Some(7), "3"),
                mk_issue(4, Status::Open, None, "4"),
            ],
            vec![],
        ));
        assert_eq!(m.plan_progress(7), (1, 3));
        assert_eq!(m.plan_progress(8), (0, 0));
    }

    #[test]
    fn milestones_tab_enter_opens_milestone_detail() {
        let mut m = DashboardModel::new();
        m.init(snap(vec![], vec![]));
        m.milestones = vec![(mk_container(4), 0)];
        m.handle_key(KeyCode::Char('3'));
        m.handle_key(KeyCode::Enter);
        assert_eq!(m.view, View::MilestoneDetail { milestone_id: 4 });
        m.handle_key(KeyCode::Esc);
        assert_eq!(m.view, View::Milestones);
    }

    #[test]
    fn milestone_detail_enter_opens_plan_detail() {
        let mut m = DashboardModel::new();
        m.init(snap_full(
            vec![mk_issue(1, Status::Dev, Some(7), "1")],
            vec![(mk_plan(7, Some(4), "1"), 0)],
            vec![(mk_container(4), 0)],
        ));
        m.view = View::MilestoneDetail { milestone_id: 4 };
        m.handle_key(KeyCode::Enter);
        assert_eq!(m.view, View::PlanDetail { plan_id: 7 });
        m.handle_key(KeyCode::Esc);
        assert_eq!(m.view, View::Plans);
    }

    #[test]
    fn number_keys_from_detail_switch_tab() {
        let mut m = DashboardModel::new();
        m.init(snap(vec![mk_issue(1, Status::Open, None, "1")], vec![]));
        m.handle_key(KeyCode::Enter); // IssueDetail
        assert_eq!(m.view, View::IssueDetail { id: 1 });
        m.handle_key(KeyCode::Char('2'));
        assert_eq!(m.view, View::Plans);
    }

    #[test]
    fn milestone_detail_hold_expires_back_to_plans() {
        let mut m = DashboardModel::new();
        m.init(snap(vec![], vec![]));
        m.milestones = vec![(mk_container(4), 0)];
        m.view = View::MilestoneDetail { milestone_id: 4 };
        m.milestone_hold = Some(2);
        enable_auto(&mut m);
        let s4 = snap_full(vec![], vec![], vec![(mk_container(4), 0)]);
        m.refresh(&s4); // 2 → 1，仍显示
        assert_eq!(m.view, View::MilestoneDetail { milestone_id: 4 });
        m.refresh(&s4); // 1 → 0 → 回 Plans tab
        assert_eq!(m.view, View::Plans);
        assert_eq!(m.milestone_hold, None);
    }

    #[test]
    fn plan_end_shows_milestone_detail_then_plans() {
        let mut m = DashboardModel::new();
        m.init(snap(vec![], vec![(mk_plan(7, Some(4), "1"), 0)]));
        m.milestones = vec![(mk_container(4), 0)];
        enable_auto(&mut m);
        let m4 = vec![(mk_container(4), 0)];
        // plan 执行中 → 自动切 plan 详情
        m.refresh(&snap_full(
            vec![mk_issue(1, Status::Dev, Some(7), "11:00")],
            vec![(mk_plan(7, Some(4), "1"), 0)],
            m4.clone(),
        ));
        assert_eq!(m.view, View::PlanDetail { plan_id: 7 });
        let end = snap_full(
            vec![mk_issue(1, Status::Done, Some(7), "12:00")],
            vec![(mk_plan(7, Some(4), "1"), 0)],
            m4.clone(),
        );
        // 间隔 ≥5 tick 后 plan 结束 → 切所属 milestone 详情，hold 启动
        for _ in 0..5 {
            m.refresh(&end);
        }
        assert_eq!(m.view, View::MilestoneDetail { milestone_id: 4 });
        assert!(m.milestone_hold.is_some());
        // 倒计时归零 + 间隔 → 回 Plans tab
        for _ in 0..8 {
            m.refresh(&end);
        }
        assert_eq!(m.view, View::Plans);
        assert_eq!(m.milestone_hold, None);
    }

    #[test]
    fn user_interaction_cancels_milestone_hold() {
        let mut m = DashboardModel::new();
        m.init(snap(vec![], vec![(mk_plan(7, Some(4), "1"), 0)]));
        m.milestones = vec![(mk_container(4), 0)];
        enable_auto(&mut m);
        let m4 = vec![(mk_container(4), 0)];
        m.refresh(&snap_full(
            vec![mk_issue(1, Status::Dev, Some(7), "11:00")],
            vec![(mk_plan(7, Some(4), "1"), 0)],
            m4.clone(),
        ));
        let end = snap_full(
            vec![mk_issue(1, Status::Done, Some(7), "12:00")],
            vec![(mk_plan(7, Some(4), "1"), 0)],
            m4.clone(),
        );
        for _ in 0..5 {
            m.refresh(&end);
        }
        assert!(matches!(m.view, View::MilestoneDetail { .. }));
        assert!(m.milestone_hold.is_some());
        // 用户按键 → 接管，取消自动倒计时
        m.handle_key(KeyCode::Char('j'));
        assert_eq!(m.milestone_hold, None);
        // 后续 refresh 不再自动回
        for _ in 0..6 {
            m.refresh(&end);
        }
        assert!(matches!(m.view, View::MilestoneDetail { .. }));
    }

    #[test]
    fn pagination_with_page_size() {
        let mut m = DashboardModel::new();
        m.page_size = 2;
        m.init(snap(
            vec![
                mk_issue(1, Status::Open, None, "1"),
                mk_issue(2, Status::Open, None, "2"),
                mk_issue(3, Status::Open, None, "3"),
                mk_issue(4, Status::Open, None, "4"),
                mk_issue(5, Status::Open, None, "5"),
            ],
            vec![],
        ));
        // page_size 2 → 3 页（updated 倒序：5,4 | 3,2 | 1）
        assert_eq!(m.pages(), 3);
        assert_eq!(m.page_issues().len(), 2);
        assert_eq!(m.page_issues()[0].id, 5);
        m.handle_key(KeyCode::Char('l'));
        assert_eq!(m.page, 1);
        assert_eq!(m.page_issues()[0].id, 3);
        m.handle_key(KeyCode::PageDown);
        assert_eq!(m.page, 2);
        assert_eq!(m.page_issues().len(), 1);
        m.handle_key(KeyCode::Char('l')); // 末页无操作
        assert_eq!(m.page, 2);
        m.handle_key(KeyCode::PageUp);
        assert_eq!(m.page, 1);
        m.handle_key(KeyCode::Char('h'));
        assert_eq!(m.page, 0);
        m.handle_key(KeyCode::Char('h')); // 首页无操作
        assert_eq!(m.page, 0);
    }
}
