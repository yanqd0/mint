//! dashboard 导航与自动切换（impl DashboardModel 独立模块：tab/详情跳转 + 空闲约束自动切换）。

use crate::tui::dashboard::diff::DashboardSnapshot;
use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::types::{View, active_plans};

/// plan 执行结束 → 所属 milestone 详情的自动停留 tick 数（1 tick = 1s）。
const MILESTONE_HOLD_TICKS: u32 = 3;
/// 自动切换前置：用户空闲至少这么多 tick（1 tick = 1s）才允许自动切换。
const AUTO_SWITCH_IDLE: u32 = 5;
/// 两次自动切换的最小间隔（tick）。
const AUTO_SWITCH_GAP: u32 = 5;

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

    /// 面板自动切换（空闲/间隔前置）：tab → 执行中 plan 详情；plan 结束 → 所属 milestone 详情 → 回 Plans tab。
    pub(crate) fn switch_panel(&mut self, snap: &DashboardSnapshot) -> Option<i64> {
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
