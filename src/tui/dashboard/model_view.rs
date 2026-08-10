//! dashboard 数据/视图查询方法：可见集、分页、分组（impl DashboardModel 独立模块，控制 model.rs 体积）。

use crate::models::{Container, Issue, Status};
use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::types::View;

/// Plans 页分组：一组 panel 的数据（组标题 + plan 行引用）。
pub struct PlanGroup<'a> {
    pub title: String,
    pub plans: Vec<&'a (Container, i64)>,
}

impl DashboardModel {
    /// 当前视图作用域内的 issue（**不应用**显示筛选）。进度统计用（done/dropped 计入，
    /// 不受 list 默认只显活跃影响）。Issues tab = 全部；PlanDetail = 该 plan；
    /// MilestoneDetail = 其下 plan 的 issue（间接）+ 直属 issue（直接）。
    pub fn scope_issues(&self) -> Vec<&Issue> {
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
                    let indirect = i
                        .plan_id
                        .and_then(|pid| self.plans.iter().find(|(c, _)| c.id == pid))
                        .map(|(c, _)| c.milestone_id == Some(milestone_id))
                        .unwrap_or(false);
                    indirect || self.milestone_direct_ids(milestone_id).contains(&i.id)
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// 当前视图展示的 issue 集合（视图作用域 + list --tui 初始筛选）。
    pub fn visible_issues(&self) -> Vec<&Issue> {
        let mut v = self.scope_issues();
        // 初始筛选：all=false 排除 done/dropped（对齐 list 默认只显活跃）；其余精确匹配。
        if let Some(f) = &self.filter {
            v.retain(|i| {
                if !f.all && matches!(i.status, Status::Done | Status::Dropped) {
                    return false;
                }
                if f.status.is_some_and(|s| i.status != s) {
                    return false;
                }
                if f.label
                    .as_deref()
                    .is_some_and(|l| !i.labels.iter().any(|x| x == l))
                {
                    return false;
                }
                if f.priority.is_some_and(|p| i.priority != p) {
                    return false;
                }
                true
            });
        }
        v
    }

    /// Plans tab 行：全部 plan 按 updated_at 逆序（扁平，不按 milestone 分组）。
    pub fn visible_plans(&self) -> Vec<&(Container, i64)> {
        let mut ps: Vec<&(Container, i64)> = self.plans.iter().collect();
        ps.sort_by(|a, b| b.0.updated_at.cmp(&a.0.updated_at));
        ps
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

        // 3. 剩余 milestone（非活跃）按 updated_at 逆序；空组（无 plan）跳过，避免孤行组标题。
        let mut rest: Vec<&(Container, i64)> = self
            .milestones
            .iter()
            .filter(|(c, _)| !active_ms.contains(&c.id))
            .collect();
        rest.sort_by(|a, b| b.0.updated_at.cmp(&a.0.updated_at));
        for (ms, _) in rest {
            let plans = self.milestone_plans(ms.id);
            if plans.is_empty() {
                continue;
            }
            let title = self.milestone_title(ms.id);
            groups.push(PlanGroup { title, plans });
        }
        groups
    }

    /// milestone 标题（含 version，如 `TUI (0.4.0)`）。
    pub(crate) fn milestone_title(&self, id: i64) -> String {
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

    /// 面板切换后校正选中（selected 1-indexed，上界 len；避免越界）。
    pub(crate) fn clamp_selected(&mut self) {
        let len = self.current_page_len();
        if self.selected > len {
            self.selected = len;
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

    /// 当前页的 plan 行（Plans tab = 全部 plan 分页；MilestoneDetail = 该 milestone 下）。
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

    /// MilestoneDetail plans 面板当前页行（按 plans_page + plans_page_size 切片）。
    pub(crate) fn page_milestone_plans(&self, milestone_id: i64) -> Vec<&(Container, i64)> {
        let all = self.milestone_plans(milestone_id);
        let start = self.plans_page * self.plans_page_size;
        if start >= all.len() {
            return Vec::new();
        }
        let end = (start + self.plans_page_size).min(all.len());
        all[start..end].to_vec()
    }

    /// MilestoneDetail 直属 issues 面板当前页 id（按 issues_page + issues_page_size 切片）。
    pub(crate) fn page_milestone_direct_ids(&self, milestone_id: i64) -> Vec<i64> {
        let all = self.milestone_direct_ids(milestone_id);
        let start = self.issues_page * self.issues_page_size;
        if start >= all.len() {
            return Vec::new();
        }
        let end = (start + self.issues_page_size).min(all.len());
        all[start..end].to_vec()
    }

    /// MilestoneDetail plans 面板页数（至少 1）。
    pub(crate) fn milestone_plans_pages(&self, milestone_id: i64) -> usize {
        self.milestone_plans(milestone_id)
            .len()
            .div_ceil(self.plans_page_size)
            .max(1)
    }

    /// MilestoneDetail issues 面板页数（至少 1）。
    pub(crate) fn milestone_issues_pages(&self, milestone_id: i64) -> usize {
        self.milestone_direct_ids(milestone_id)
            .len()
            .div_ceil(self.issues_page_size)
            .max(1)
    }

    /// MilestoneDetail 当前页分段：(plans 段行数, issues 段行数)。光标路由翻页用。
    pub(crate) fn milestone_segments(&self, milestone_id: i64) -> (usize, usize) {
        (
            self.page_milestone_plans(milestone_id).len(),
            self.page_milestone_direct_ids(milestone_id).len(),
        )
    }

    /// MilestoneDetail 直属 issue id 列表（按快照顺序）。
    pub(crate) fn milestone_direct_ids(&self, milestone_id: i64) -> Vec<i64> {
        self.milestone_directs
            .iter()
            .filter(|(mid, _)| *mid == milestone_id)
            .map(|(_, iid)| *iid)
            .collect()
    }

    pub(crate) fn current_page_len(&self) -> usize {
        match self.view {
            View::Issues | View::PlanDetail { .. } => self.page_issues().len(),
            View::MilestoneDetail { milestone_id } => {
                self.page_milestone_plans(milestone_id).len()
                    + self.page_milestone_direct_ids(milestone_id).len()
            }
            View::Plans => self.page_plans().len(),
            View::Milestones => self.page_milestones().len(),
            View::IssueDetail { .. } => 0,
        }
    }

    /// 面板数据变化后校正页号（避免越界）；MilestoneDetail 对 plans/issues 双页分别夹取。
    pub(crate) fn clamp_page(&mut self) {
        match self.view {
            View::MilestoneDetail { milestone_id } => {
                self.plans_page = self
                    .plans_page
                    .min(self.milestone_plans_pages(milestone_id) - 1);
                self.issues_page = self
                    .issues_page
                    .min(self.milestone_issues_pages(milestone_id) - 1);
            }
            _ => {
                if self.page >= self.pages() {
                    self.page = self.pages().saturating_sub(1);
                }
            }
        }
    }

    /// 按 id 查 issue（详情数据源）。
    pub fn issue(&self, id: i64) -> Option<&Issue> {
        self.issues.iter().find(|i| i.id == id)
    }
}
