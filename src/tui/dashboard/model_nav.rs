//! dashboard 导航辅助（impl DashboardModel 独立模块）：tab/详情跳转、选中行归属查询。
//! 自动跳转（双 queue）见 `jump/` 子模块。

use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::types::View;

impl DashboardModel {
    /// 详情指向的实体已不存在 → 回对应 tab。
    pub(crate) fn prune_detail(&mut self) {
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

    /// 切到 tab 页（清空行状态）。
    pub(crate) fn switch_tab(&mut self, tab: View) {
        self.view = tab;
        self.page = 0;
        self.selected = 0;
    }

    /// 当前所属 tab（详情页归其 tab）。
    pub(crate) fn active_tab(&self) -> View {
        match self.view {
            View::Issues | View::IssueDetail { .. } => View::Issues,
            View::Plans | View::PlanDetail { .. } => View::Plans,
            View::Milestones | View::MilestoneDetail { .. } => View::Milestones,
        }
    }

    /// 选中行的 plan id（Issues 行的 plan_id 或 Plans 行的 plan id）。
    pub(crate) fn selected_plan_id(&self) -> Option<i64> {
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
    pub(crate) fn selected_milestone_id(&self) -> Option<i64> {
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
}
