//! 合并器：queue1 延迟读空 → 同类聚合 → queue2（限容挤队首）。

use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::types::{
    JUMP_MERGE_DELAY, JUMP_QUEUE_LIMIT, JumpKind, JumpRequest, JumpTarget, RawJump,
};

impl DashboardModel {
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
            self.push_ready(req);
        }
    }

    /// queue2 入队（满则挤掉队首，即最旧请求）。
    fn push_ready(&mut self, req: JumpRequest) {
        if self.ready.len() >= JUMP_QUEUE_LIMIT {
            self.ready.pop_front();
        }
        self.ready.push_back(req);
    }
}

/// 同类聚合：列表请求去重合并（含全部闪烁目标），详情请求按序保留。
pub(crate) fn merge_raw(raw: &[RawJump]) -> Vec<JumpRequest> {
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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(m.ready.front().unwrap().target, JumpTarget::PlanDetail(2));
    }
}
