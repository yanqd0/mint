//! dashboard 全量快照 + 变化 diff（纯函数，无 ratatui 依赖，可独立单测）。

use std::collections::{HashMap, HashSet};

use crate::models::{Container, ContainerStatus, Issue, Status};

/// 一次全量快照（dashboard 每 tick 拉取：当前项目 issue + 全部 plan + 全部 milestone）。
#[derive(Debug, Clone)]
pub struct DashboardSnapshot {
    pub issues: Vec<Issue>,
    pub plans: Vec<(Container, i64)>,
    pub milestones: Vec<(Container, i64)>,
    /// 当前项目名（外框标题用）。
    pub project: String,
    /// milestone 直属 issue 关联（milestone_id, issue_id），详情页直属 issue 列表用。
    pub milestone_directs: Vec<(i64, i64)>,
}

impl DashboardSnapshot {
    pub fn issue(&self, id: i64) -> Option<&Issue> {
        self.issues.iter().find(|i| i.id == id)
    }
    pub fn plan(&self, id: i64) -> Option<&(Container, i64)> {
        self.plans.iter().find(|(c, _)| c.id == id)
    }
    pub fn milestone(&self, id: i64) -> Option<&(Container, i64)> {
        self.milestones.iter().find(|(c, _)| c.id == id)
    }
}

/// 会话内变化事件（由两轮快照 diff 产生）。
#[derive(Debug, Clone)]
pub enum ChangeEvent {
    IssueAdded {
        issue: Issue,
    },
    IssueStatusChanged {
        issue: Issue,
        from: Status,
        to: Status,
    },
    IssueUpdated {
        issue: Issue,
    },
    IssueRemoved {
        id: i64,
        title: String,
    },
    PlanAdded {
        plan: Container,
        count: i64,
    },
    PlanStatusChanged {
        plan: Container,
        from: ContainerStatus,
        to: ContainerStatus,
    },
    PlanUpdated {
        plan: Container,
    },
    PlanRemoved {
        id: i64,
        title: String,
    },
    MilestoneAdded {
        milestone: Container,
        count: i64,
    },
    MilestoneUpdated {
        milestone: Container,
    },
    /// milestone 直属 issue 挂载变化（attach/detach，milestone 本体字段不变）。
    MilestoneDirectChanged {
        milestone_id: i64,
        count: i64,
    },
}

impl ChangeEvent {
    /// 若为 issue 相关事件，返回 issue id。
    pub fn issue_id(&self) -> Option<i64> {
        match self {
            ChangeEvent::IssueAdded { issue }
            | ChangeEvent::IssueStatusChanged { issue, .. }
            | ChangeEvent::IssueUpdated { issue } => Some(issue.id),
            ChangeEvent::IssueRemoved { id, .. } => Some(*id),
            _ => None,
        }
    }
    /// 若为 plan 相关事件，返回 plan id。
    pub fn plan_id(&self) -> Option<i64> {
        match self {
            ChangeEvent::PlanAdded { plan, .. }
            | ChangeEvent::PlanStatusChanged { plan, .. }
            | ChangeEvent::PlanUpdated { plan } => Some(plan.id),
            ChangeEvent::PlanRemoved { id, .. } => Some(*id),
            _ => None,
        }
    }
}

/// issue 是否发生非状态字段变化（忽略 hit_count/updated_at/status 噪声）。
fn issue_fields_changed(a: &Issue, b: &Issue) -> bool {
    a.title != b.title
        || a.kind != b.kind
        || a.priority != b.priority
        || a.plan_id != b.plan_id
        || a.body != b.body
        || a.labels != b.labels
        || a.links != b.links
}

/// plan 是否发生非状态字段变化（title/body/milestone_id；#334 补 PlanUpdated 事件）。
fn plan_fields_changed(a: &Container, b: &Container) -> bool {
    a.title != b.title || a.body != b.body || a.milestone_id != b.milestone_id
}

/// 两轮快照 → 变化事件（issues 按 id 升序 → plans 按 id 升序，确定性）。
pub fn diff_snapshots(prev: &DashboardSnapshot, next: &DashboardSnapshot) -> Vec<ChangeEvent> {
    let prev_issues: HashMap<i64, &Issue> = prev.issues.iter().map(|i| (i.id, i)).collect();
    let prev_plans: HashMap<i64, &(Container, i64)> =
        prev.plans.iter().map(|p| (p.0.id, p)).collect();
    let mut events = Vec::new();

    for issue in &next.issues {
        match prev_issues.get(&issue.id) {
            None => events.push(ChangeEvent::IssueAdded {
                issue: issue.clone(),
            }),
            Some(p) => {
                if p.status != issue.status {
                    events.push(ChangeEvent::IssueStatusChanged {
                        issue: issue.clone(),
                        from: p.status,
                        to: issue.status,
                    });
                } else if issue_fields_changed(p, issue) {
                    events.push(ChangeEvent::IssueUpdated {
                        issue: issue.clone(),
                    });
                }
            }
        }
    }
    let next_ids: HashSet<i64> = next.issues.iter().map(|i| i.id).collect();
    for (id, p) in &prev_issues {
        if !next_ids.contains(id) {
            events.push(ChangeEvent::IssueRemoved {
                id: *id,
                title: p.title.clone(),
            });
        }
    }

    for (plan, count) in &next.plans {
        match prev_plans.get(&plan.id) {
            None => events.push(ChangeEvent::PlanAdded {
                plan: plan.clone(),
                count: *count,
            }),
            Some((p, _)) => {
                if p.status != plan.status {
                    events.push(ChangeEvent::PlanStatusChanged {
                        plan: plan.clone(),
                        from: p.status,
                        to: plan.status,
                    });
                } else if plan_fields_changed(p, plan) {
                    events.push(ChangeEvent::PlanUpdated { plan: plan.clone() });
                }
            }
        }
    }
    let next_plan_ids: HashSet<i64> = next.plans.iter().map(|(c, _)| c.id).collect();
    for (id, (p, _)) in &prev_plans {
        if !next_plan_ids.contains(id) {
            events.push(ChangeEvent::PlanRemoved {
                id: *id,
                title: p.title.clone(),
            });
        }
    }

    // milestone 新增（规则 8）与内容更新（#137：字段编辑/状态改变 → 跳详情）。
    let prev_ms: HashMap<i64, &Container> =
        prev.milestones.iter().map(|(c, _)| (c.id, c)).collect();
    for (ms, count) in &next.milestones {
        match prev_ms.get(&ms.id) {
            None => events.push(ChangeEvent::MilestoneAdded {
                milestone: ms.clone(),
                count: *count,
            }),
            Some(p) => {
                if p.title != ms.title
                    || p.version != ms.version
                    || p.body != ms.body
                    || p.status != ms.status
                {
                    events.push(ChangeEvent::MilestoneUpdated {
                        milestone: ms.clone(),
                    });
                }
            }
        }
    }

    // milestone 直属挂载变化（#335：direct attach/detach 不改 milestone 本体字段，
    // 需单独比较；count 变化即触发，跳转详情）。
    let direct_count =
        |v: &[(i64, i64)], mid: i64| -> usize { v.iter().filter(|(m, _)| *m == mid).count() };
    let prev_direct_ms: HashSet<i64> = prev.milestone_directs.iter().map(|(m, _)| *m).collect();
    let next_direct_ms: HashSet<i64> = next.milestone_directs.iter().map(|(m, _)| *m).collect();
    // union 收集后按 milestone_id 排序，保证事件顺序确定（#335；HashSet 迭代无序）。
    let mut changed_ms: Vec<i64> = prev_direct_ms.union(&next_direct_ms).copied().collect();
    changed_ms.sort_unstable();
    for mid in changed_ms {
        let prev_c = direct_count(&prev.milestone_directs, mid);
        let next_c = direct_count(&next.milestone_directs, mid);
        if prev_c != next_c {
            events.push(ChangeEvent::MilestoneDirectChanged {
                milestone_id: mid,
                count: next_c as i64,
            });
        }
    }

    events
}

#[cfg(test)]
#[path = "diff_tests.rs"]
mod tests;
