//! 执行器：空闲/间隔满足执行 queue2 队首（跳转 + 闪烁）+ 闪烁管理。

use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::types::{FLASH_TICKS, FlashItem, JumpTarget, View};

/// 自动切换前置：用户空闲至少这么多 tick（1 tick = 1s）才允许自动跳转。
/// 10s：人类操作（含搜索输入）后 5s 太短，易在思考间隙被自动跳转打断。
const AUTO_SWITCH_IDLE: u32 = 10;
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
        self.navigate_auto(view_from_target(target));
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
        m.user_idle = AUTO_SWITCH_IDLE; // 临界值 10
        m.auto_last = AUTO_SWITCH_GAP;
        assert_eq!(m.execute_jump(), Some(JumpTarget::Plans));
        assert_eq!(m.view, View::Plans);
    }

    /// #336：idle≥60s 且本 tick 执行了 auto-jump 时，refresh 不撤销跳转。
    /// prev 空 + next 有 plan → PlanAdded 事件 → queue 合并 → execute_jump，
    /// 修复前同 tick 的 home_timeout（idle 达标且 queue 已空）会切回 Issues。
    #[test]
    fn auto_jump_not_reverted_by_home_timeout_same_tick() {
        use crate::models::Container;
        use crate::tui::dashboard::types::View;
        let mut m = DashboardModel::new();
        // prev：空快照。
        m.init(snap());
        // idle 已超 home 阈值（60s），且 auto-jump 条件满足。
        m.user_idle = 100;
        m.auto_last = AUTO_SWITCH_GAP;
        // next：新增一个 plan → PlanAdded 事件。
        let next = DashboardSnapshot {
            plans: vec![(
                Container {
                    id: 1,
                    title: "sprint".into(),
                    version: None,
                    body: None,
                    milestone_id: None,
                    status: crate::models::ContainerStatus::Open,
                    created_at: "t".into(),
                    updated_at: "t".into(),
                },
                0,
            )],
            ..snap()
        };
        let r = m.refresh(&next);
        assert!(r.jumped.is_some(), "应执行 auto-jump: {:?}", r.jumped);
        // 修复前：home_timeout 在 jumped 后同 tick 切回 Issues；修复后保持 Plans。
        assert_eq!(
            m.view,
            View::Plans,
            "auto-jump 不应被同 tick home_timeout 撤销"
        );
    }

    #[test]
    fn execute_sets_flash_with_ticks() {
        let mut m = DashboardModel::new();
        m.init(snap());
        m.user_idle = AUTO_SWITCH_IDLE;
        m.auto_last = AUTO_SWITCH_GAP;
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
