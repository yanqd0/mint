//! dashboard 导航与自动跳转（impl DashboardModel 独立模块）。
//! 双 queue 管道：事件 → queue1(原始) → 合并器(延迟读空) → queue2(就绪) → 执行器(每 5s) → UI。

use crate::tui::dashboard::diff::ChangeEvent;
use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::types::{
    FLASH_TICKS, FlashItem, JUMP_MERGE_DELAY, JUMP_QUEUE_LIMIT, JumpKind, JumpRequest, JumpTarget,
    RawJump, View,
};

/// 自动切换前置：用户空闲至少这么多 tick（1 tick = 1s）才允许自动跳转。
const AUTO_SWITCH_IDLE: u32 = 5;
/// 两次自动跳转的最小间隔（tick）。
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

    /// 合并器：queue1 非空则延迟 JUMP_MERGE_DELAY tick 后读空，同类聚合 → queue2（限容挤队首）。
    pub(crate) fn merge_jumps(&mut self) {
        if self.pending.is_empty() {
            self.merge_delay = 0;
            return;
        }
        self.merge_delay = self.merge_delay.saturating_add(1);
        if self.merge_delay < JUMP_MERGE_DELAY {
            return;
        }
        let raw: Vec<RawJump> = self.pending.drain(..).collect();
        self.merge_delay = 0;
        for req in merge_raw(&raw) {
            if self.ready.len() >= JUMP_QUEUE_LIMIT {
                self.ready.pop_front(); // 新入队挤掉队首（最旧）
            }
            self.ready.push_back(req);
        }
    }

    /// 执行器：空闲/间隔满足且 queue2 非空 → 执行队首（跳转 + 闪烁）。
    pub(crate) fn execute_jump(&mut self) -> Option<JumpTarget> {
        if self.user_idle < AUTO_SWITCH_IDLE || self.auto_last < AUTO_SWITCH_GAP {
            return None;
        }
        let req = self.ready.pop_front()?;
        let target = req.target;
        self.view = view_from_target(target);
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
        self.page = 0;
        self.selected = 0;
        Some(target)
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

/// 事件 → 原始跳转请求（基础映射；milestone 归属查询在 #108 完善）。
pub(crate) fn raw_jumps_from_events(events: &[ChangeEvent]) -> Vec<RawJump> {
    let mut out = Vec::new();
    for ev in events {
        match ev {
            ChangeEvent::IssueAdded { issue } => {
                out.push(RawJump {
                    target: JumpTarget::Issues,
                });
                out.push(RawJump {
                    target: JumpTarget::IssueDetail(issue.id),
                });
            }
            ChangeEvent::IssueStatusChanged { issue, .. } => {
                // 规则 5：有 plan 跳 plan 详情（无 plan 时 #108 完善 milestone/详情）。
                if let Some(pid) = issue.plan_id {
                    out.push(RawJump {
                        target: JumpTarget::PlanDetail(pid),
                    });
                }
            }
            ChangeEvent::PlanAdded { plan, .. } => {
                out.push(RawJump {
                    target: JumpTarget::Plans,
                });
                out.push(RawJump {
                    target: JumpTarget::PlanDetail(plan.id),
                });
            }
            _ => {}
        }
    }
    out
}

/// 同类聚合：列表请求去重合并（含全部闪烁目标），详情请求按序保留。
fn merge_raw(raw: &[RawJump]) -> Vec<JumpRequest> {
    let mut list_issues = false;
    let mut list_plans = false;
    let mut list_milestones = false;
    let mut flash_issue: Vec<(i64, JumpKind)> = Vec::new();
    let mut flash_plan: Vec<(i64, JumpKind)> = Vec::new();
    let mut flash_milestone: Vec<(i64, JumpKind)> = Vec::new();
    let mut details: Vec<JumpTarget> = Vec::new();
    for r in raw {
        match r.target {
            JumpTarget::Issues => list_issues = true,
            JumpTarget::Plans => list_plans = true,
            JumpTarget::Milestones => list_milestones = true,
            JumpTarget::IssueDetail(id) => {
                flash_issue.push((id, JumpKind::Issue));
                details.push(JumpTarget::IssueDetail(id));
            }
            JumpTarget::PlanDetail(id) => {
                flash_plan.push((id, JumpKind::Plan));
                details.push(JumpTarget::PlanDetail(id));
            }
            JumpTarget::MilestoneDetail(id) => {
                flash_milestone.push((id, JumpKind::Milestone));
                details.push(JumpTarget::MilestoneDetail(id));
            }
        }
    }
    let mut out = Vec::new();
    if list_plans {
        out.push(JumpRequest {
            target: JumpTarget::Plans,
            flash: flash_plan,
        });
    }
    if list_milestones {
        out.push(JumpRequest {
            target: JumpTarget::Milestones,
            flash: flash_milestone,
        });
    }
    if list_issues {
        out.push(JumpRequest {
            target: JumpTarget::Issues,
            flash: flash_issue,
        });
    }
    for d in details {
        out.push(JumpRequest {
            target: d,
            flash: Vec::new(),
        });
    }
    out
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
    use crate::models::{Container, Issue, Kind, Status};
    use crate::tui::dashboard::diff::DashboardSnapshot;

    fn mk_issue(id: i64, status: Status, plan_id: Option<i64>) -> Issue {
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
            updated_at: "t".into(),
        }
    }

    fn snap(issues: Vec<Issue>, plans: Vec<(Container, i64)>) -> DashboardSnapshot {
        DashboardSnapshot {
            issues,
            plans,
            milestones: vec![],
            project: "mint".into(),
        }
    }

    #[test]
    fn events_produce_list_then_detail_jumps() {
        let events = [ChangeEvent::IssueAdded {
            issue: mk_issue(3, Status::Open, None),
        }];
        let raw = raw_jumps_from_events(&events);
        assert_eq!(raw.len(), 2);
        assert_eq!(raw[0].target, JumpTarget::Issues);
        assert_eq!(raw[1].target, JumpTarget::IssueDetail(3));
    }

    #[test]
    fn merge_dedupes_list_and_keeps_details() {
        let raw = [
            RawJump {
                target: JumpTarget::Plans,
            },
            RawJump {
                target: JumpTarget::PlanDetail(7),
            },
            RawJump {
                target: JumpTarget::PlanDetail(8),
            },
        ];
        let merged = merge_raw(&raw);
        assert_eq!(merged.len(), 3);
        assert_eq!(merged[0].target, JumpTarget::Plans);
        assert_eq!(merged[0].flash.len(), 2); // 两个 plan 闪烁目标
        assert_eq!(merged[1].target, JumpTarget::PlanDetail(7));
        assert_eq!(merged[2].target, JumpTarget::PlanDetail(8));
    }

    #[test]
    fn ready_queue_drops_oldest_when_full() {
        let mut m = DashboardModel::new();
        m.init(snap(vec![], vec![]));
        for i in 0..JUMP_QUEUE_LIMIT + 2 {
            m.ready.push_back(JumpRequest {
                target: JumpTarget::PlanDetail(i as i64),
                flash: vec![],
            });
            if m.ready.len() > JUMP_QUEUE_LIMIT {
                m.ready.pop_front();
            }
        }
        assert_eq!(m.ready.len(), JUMP_QUEUE_LIMIT);
        // 队首应是最新的 i=2（挤掉了 0/1）
        assert_eq!(m.ready.front().unwrap().target, JumpTarget::PlanDetail(2));
    }

    #[test]
    fn execute_jump_requires_idle_gap() {
        let mut m = DashboardModel::new();
        m.init(snap(vec![], vec![]));
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
}
