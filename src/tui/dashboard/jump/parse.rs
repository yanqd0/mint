//! 事件 → 原始跳转请求（规则 4/5/6/8）。

use crate::models::ContainerStatus;
use crate::tui::dashboard::diff::ChangeEvent;
use crate::tui::dashboard::types::{JumpTarget, RawJump};

/// 事件 → queue1 原始请求。
/// - 规则 4/8：新增 issue/plan/milestone → 列表 + 详情
/// - 规则 5：issue 状态变化 → 有 plan 跳 plan；无 plan 跳 issue 详情（直属 milestone 待数据扩展）
/// - 规则 6：plan 结束（done）→ 所属 milestone（无 milestone 不跳）
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
            ChangeEvent::IssueStatusChanged { issue, .. } => match issue.plan_id {
                Some(pid) => out.push(RawJump {
                    target: JumpTarget::PlanDetail(pid),
                }),
                // 无 plan：直属 milestone（#258）→ milestone 详情；否则兜底 issues 列表。
                None => match issue.direct_milestone {
                    Some(mid) => out.push(RawJump {
                        target: JumpTarget::MilestoneDetail(mid),
                    }),
                    None => out.push(RawJump {
                        target: JumpTarget::Issues,
                    }),
                },
            },
            ChangeEvent::IssueUpdated { issue } => {
                out.push(RawJump {
                    target: JumpTarget::IssueDetail(issue.id),
                });
            }
            ChangeEvent::PlanUpdated { plan } => {
                out.push(RawJump {
                    target: JumpTarget::PlanDetail(plan.id),
                });
            }
            ChangeEvent::PlanAdded { plan, .. } => {
                out.push(RawJump {
                    target: JumpTarget::Plans,
                });
                out.push(RawJump {
                    target: JumpTarget::PlanDetail(plan.id),
                });
            }
            ChangeEvent::PlanStatusChanged { plan, to, .. } => {
                if *to == ContainerStatus::Done
                    && let Some(mid) = plan.milestone_id
                {
                    out.push(RawJump {
                        target: JumpTarget::MilestoneDetail(mid),
                    });
                }
            }
            ChangeEvent::MilestoneAdded { milestone, .. } => {
                out.push(RawJump {
                    target: JumpTarget::Milestones,
                });
                out.push(RawJump {
                    target: JumpTarget::MilestoneDetail(milestone.id),
                });
            }
            ChangeEvent::MilestoneUpdated { milestone } => {
                out.push(RawJump {
                    target: JumpTarget::MilestoneDetail(milestone.id),
                });
            }
            // #335：direct attach/detach 跳转对应 milestone 详情。
            ChangeEvent::MilestoneDirectChanged { milestone_id, .. } => {
                out.push(RawJump {
                    target: JumpTarget::MilestoneDetail(*milestone_id),
                });
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Container, Issue, Kind, Status};

    fn mk_issue(id: i64, status: Status, plan_id: Option<i64>) -> Issue {
        Issue {
            id,
            title: "t".into(),
            body: None,
            kind: Kind::Problem,
            status,
            priority: 3,
            project: Some("mint".into()),
            test_cmd: None,
            dropped_reason: None,
            last_commit_id: None,
            plan_id,
            direct_milestone: None,
            machine_id: None,
            uid: None,
            hit_count: 0,
            label_colors: std::collections::HashMap::new(),
            labels: vec![],
            links: vec![],
            created_at: "t".into(),
            updated_at: "t".into(),
        }
    }

    fn mk_plan(id: i64, milestone: Option<i64>) -> Container {
        Container {
            id,
            title: "p".into(),
            version: None,
            body: None,
            milestone_id: milestone,
            status: ContainerStatus::Open,
            created_at: "t".into(),
            updated_at: "t".into(),
        }
    }

    fn mk_milestone(id: i64) -> Container {
        Container {
            id,
            title: "m".into(),
            version: Some("0.1.0".into()),
            body: None,
            milestone_id: None,
            status: ContainerStatus::Open,
            created_at: "t".into(),
            updated_at: "t".into(),
        }
    }

    #[test]
    fn issue_added_jumps_list_then_detail() {
        let ev = [ChangeEvent::IssueAdded {
            issue: mk_issue(3, Status::Open, None),
        }];
        let raw = raw_jumps_from_events(&ev);
        assert_eq!(raw[0].target, JumpTarget::Issues);
        assert_eq!(raw[1].target, JumpTarget::IssueDetail(3));
    }

    #[test]
    fn status_change_jumps_plan_or_issue() {
        let with_plan = [ChangeEvent::IssueStatusChanged {
            issue: mk_issue(1, Status::Dev, Some(7)),
            from: Status::Planned,
            to: Status::Dev,
        }];
        assert_eq!(
            raw_jumps_from_events(&with_plan)[0].target,
            JumpTarget::PlanDetail(7)
        );
        let no_plan = [ChangeEvent::IssueStatusChanged {
            issue: mk_issue(1, Status::Dev, None),
            from: Status::Planned,
            to: Status::Dev,
        }];
        assert_eq!(
            raw_jumps_from_events(&no_plan)[0].target,
            JumpTarget::Issues
        );
    }

    #[test]
    fn status_change_no_plan_direct_milestone_jumps_milestone() {
        let mut issue = mk_issue(2, Status::Dev, None);
        issue.direct_milestone = Some(9); // 无 plan 但直属挂 milestone（#258）
        let ev = [ChangeEvent::IssueStatusChanged {
            issue,
            from: Status::Planned,
            to: Status::Dev,
        }];
        assert_eq!(
            raw_jumps_from_events(&ev)[0].target,
            JumpTarget::MilestoneDetail(9)
        );
    }

    #[test]
    fn issue_updated_jumps_detail() {
        let ev = [ChangeEvent::IssueUpdated {
            issue: mk_issue(5, Status::Open, None),
        }];
        assert_eq!(
            raw_jumps_from_events(&ev)[0].target,
            JumpTarget::IssueDetail(5)
        );
    }

    #[test]
    fn plan_updated_jumps_plan_detail() {
        let ev = [ChangeEvent::PlanUpdated {
            plan: mk_plan(7, Some(4)),
        }];
        assert_eq!(
            raw_jumps_from_events(&ev)[0].target,
            JumpTarget::PlanDetail(7)
        );
    }

    #[test]
    fn plan_done_jumps_milestone() {
        let ev = [ChangeEvent::PlanStatusChanged {
            plan: mk_plan(7, Some(4)),
            from: ContainerStatus::Running,
            to: ContainerStatus::Done,
        }];
        let raw = raw_jumps_from_events(&ev);
        assert_eq!(raw[0].target, JumpTarget::MilestoneDetail(4));
    }

    #[test]
    fn plan_done_without_milestone_no_jump() {
        let ev = [ChangeEvent::PlanStatusChanged {
            plan: mk_plan(7, None),
            from: ContainerStatus::Running,
            to: ContainerStatus::Done,
        }];
        assert!(raw_jumps_from_events(&ev).is_empty());
    }

    #[test]
    fn milestone_added_jumps_list_then_detail() {
        let ev = [ChangeEvent::MilestoneAdded {
            milestone: mk_milestone(9),
            count: 0,
        }];
        let raw = raw_jumps_from_events(&ev);
        assert_eq!(raw[0].target, JumpTarget::Milestones);
        assert_eq!(raw[1].target, JumpTarget::MilestoneDetail(9));
    }

    #[test]
    fn milestone_updated_jumps_detail() {
        let ev = [ChangeEvent::MilestoneUpdated {
            milestone: mk_milestone(9),
        }];
        assert_eq!(
            raw_jumps_from_events(&ev)[0].target,
            JumpTarget::MilestoneDetail(9)
        );
    }
}
