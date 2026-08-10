//! 执行器：空闲/间隔满足执行 queue2 队首（跳转 + 闪烁）+ 闪烁管理。

use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::types::{FLASH_TICKS, FlashItem, JumpTarget, View};

/// 自动切换前置：用户空闲至少这么多 tick（1 tick = 1s）才允许自动跳转。
const AUTO_SWITCH_IDLE: u32 = 5;
/// 两次自动跳转的最小间隔（tick）。
const AUTO_SWITCH_GAP: u32 = 5;

impl DashboardModel {
    /// 执行器：空闲/间隔满足且 queue2 非空 → 执行队首（跳转 + 闪烁）。
    pub(crate) fn execute_jump(&mut self) -> Option<JumpTarget> {
        if self.user_idle < AUTO_SWITCH_IDLE || self.auto_last < AUTO_SWITCH_GAP {
            return None;
        }
        let req = self.ready.pop_front()?;
        let target = req.target;
        self.navigate(view_from_target(target));
        self.flash = req
            .flash
            .into_iter()
            .map(|(id, kind)| FlashItem {
                id,
                kind,
                ticks: FLASH_TICKS,
            })
            .collect();
        self.auto_last = 0;
        Some(target)
    }

    /// 闪烁递减（过期清除），每 tick 调用。
    pub(crate) fn tick_flash(&mut self) {
        for f in &mut self.flash {
            f.ticks = f.ticks.saturating_sub(1);
        }
        self.flash.retain(|f| f.ticks > 0);
    }
}

/// JumpTarget → View。
fn view_from_target(t: JumpTarget) -> View {
    match t {
        JumpTarget::Issues => View::Issues,
        JumpTarget::Plans => View::Plans,
        JumpTarget::Milestones => View::Milestones,
        JumpTarget::IssueDetail(id) => View::IssueDetail { id },
        JumpTarget::PlanDetail(id) => View::PlanDetail { plan_id: id },
        JumpTarget::MilestoneDetail(id) => View::MilestoneDetail { milestone_id: id },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::dashboard::diff::DashboardSnapshot;
    use crate::tui::dashboard::types::{JumpKind, JumpRequest};

    fn snap() -> DashboardSnapshot {
        DashboardSnapshot {
            issues: vec![],
            plans: vec![],
            milestones: vec![],
            project: "mint".into(),
            milestone_directs: vec![],
        }
    }

    #[test]
    fn execute_jump_requires_idle_gap() {
        let mut m = DashboardModel::new();
        m.init(snap());
        m.ready.push_back(JumpRequest {
            target: JumpTarget::Plans,
            flash: vec![],
        });
        assert_eq!(m.execute_jump(), None); // 空闲不足
        m.user_idle = 5;
        m.auto_last = 5;
        assert_eq!(m.execute_jump(), Some(JumpTarget::Plans));
        assert_eq!(m.view, View::Plans);
    }

    #[test]
    fn execute_sets_flash_with_ticks() {
        let mut m = DashboardModel::new();
        m.init(snap());
        m.user_idle = 5;
        m.auto_last = 5;
        m.ready.push_back(JumpRequest {
            target: JumpTarget::Plans,
            flash: vec![(7, JumpKind::Plan)],
        });
        m.execute_jump();
        assert_eq!(m.flash.len(), 1);
        assert_eq!(m.flash[0].ticks, FLASH_TICKS);
        m.tick_flash();
        assert_eq!(m.flash[0].ticks, FLASH_TICKS - 1);
        // 归零后过期清除。
        m.tick_flash();
        assert!(m.flash.is_empty());
    }
}
