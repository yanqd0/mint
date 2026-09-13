//! dashboard 分页/游标视图方法（impl DashboardModel 独立模块，控制 model_view.rs 体积）。

use crate::models::{Container, Issue};

use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::types::View;

impl DashboardModel {
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

    /// MilestoneDetail issues 面板当前页 issue（全部 issue：直属在前 + 间接；按 issues_page + issues_page_size 切片）。
    /// 数据源 `scope_issues()` 按 `self.view` 取 milestone，参数仅作签名一致性。
    pub(crate) fn page_milestone_issues(&self, _milestone_id: i64) -> Vec<&Issue> {
        let all = self.scope_issues();
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

    /// MilestoneDetail issues 面板页数（至少 1，按全部 issue 计数）。
    pub(crate) fn milestone_issues_pages(&self, _milestone_id: i64) -> usize {
        self.scope_issues()
            .len()
            .div_ceil(self.issues_page_size)
            .max(1)
    }

    /// MilestoneDetail 当前页分段：(plans 段行数, issues 段行数)。光标路由翻页用。
    pub(crate) fn milestone_segments(&self, milestone_id: i64) -> (usize, usize) {
        (
            self.page_milestone_plans(milestone_id).len(),
            self.page_milestone_issues(milestone_id).len(),
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
                    + self.page_milestone_issues(milestone_id).len()
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
