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
mod tests {
    use super::*;
    use crate::models::{ContainerStatus, Kind};

    fn mk_issue(id: i64, title: &str, status: Status) -> Issue {
        Issue {
            id,
            title: title.into(),
            body: None,
            kind: Kind::Problem,
            status,
            priority: 3,
            project_id: 1,
            project: Some("mint".into()),
            test_cmd: None,
            dropped_reason: None,
            last_commit_id: None,
            plan_id: None,
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

    fn mk_container(id: i64, title: &str, status: ContainerStatus) -> Container {
        Container {
            id,
            title: title.into(),
            version: None,
            body: None,
            milestone_id: None,
            status,
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
            milestone_directs: vec![],
        }
    }

    #[test]
    fn empty_to_empty_no_events() {
        let s = snap(vec![], vec![]);
        assert!(diff_snapshots(&s, &s).is_empty());
    }

    #[test]
    fn issue_added_and_removed() {
        let prev = snap(vec![], vec![]);
        let next = snap(vec![mk_issue(1, "hello", Status::Open)], vec![]);
        let ev = diff_snapshots(&prev, &next);
        assert_eq!(ev.len(), 1);
        assert_eq!(ev[0].issue_id(), Some(1));
        assert!(matches!(ev[0], ChangeEvent::IssueAdded { .. }));

        let ev2 = diff_snapshots(&next, &prev);
        assert_eq!(ev2.len(), 1);
        match &ev2[0] {
            ChangeEvent::IssueRemoved { id, title } => {
                assert_eq!(*id, 1);
                assert_eq!(title, "hello");
            }
            other => panic!("应 IssueRemoved: {other:?}"),
        }
    }

    #[test]
    fn issue_status_change_reports_from_to() {
        let prev = snap(vec![mk_issue(1, "a", Status::Open)], vec![]);
        let next_issue = mk_issue(1, "a", Status::Dev);
        let next = snap(vec![next_issue.clone()], vec![]);
        let ev = diff_snapshots(&prev, &next);
        assert_eq!(ev.len(), 1);
        match &ev[0] {
            ChangeEvent::IssueStatusChanged { issue, from, to } => {
                assert_eq!(issue.id, 1);
                assert_eq!(*from, Status::Open);
                assert_eq!(*to, Status::Dev);
            }
            other => panic!("应 IssueStatusChanged: {other:?}"),
        }
    }

    #[test]
    fn issue_field_update_ignores_hit_count_and_updated_at() {
        let prev_issue = mk_issue(1, "a", Status::Open);
        let mut next_issue = mk_issue(1, "a", Status::Open);
        next_issue.hit_count = 5;
        next_issue.updated_at = "later".into();
        // 仅 hit_count/updated_at 变化 → 无事件
        assert!(
            diff_snapshots(
                &snap(vec![prev_issue.clone()], vec![]),
                &snap(vec![next_issue.clone()], vec![])
            )
            .is_empty()
        );
        // title 变化 → IssueUpdated
        next_issue.title = "changed".into();
        let ev = diff_snapshots(
            &snap(vec![prev_issue.clone()], vec![]),
            &snap(vec![next_issue], vec![]),
        );
        assert_eq!(ev.len(), 1);
        assert!(matches!(ev[0], ChangeEvent::IssueUpdated { .. }));
    }

    #[test]
    fn plan_added_and_status_change() {
        let prev = snap(
            vec![],
            vec![(mk_container(1, "p", ContainerStatus::Open), 0)],
        );
        let next = snap(
            vec![],
            vec![(mk_container(1, "p", ContainerStatus::Running), 2)],
        );
        let ev = diff_snapshots(&prev, &next);
        assert_eq!(ev.len(), 1);
        match &ev[0] {
            ChangeEvent::PlanStatusChanged { plan, from, to } => {
                assert_eq!(plan.id, 1);
                assert_eq!(*from, ContainerStatus::Open);
                assert_eq!(*to, ContainerStatus::Running);
            }
            other => panic!("应 PlanStatusChanged: {other:?}"),
        }
        // 新增 plan
        let prev2 = snap(vec![], vec![]);
        let next2 = snap(
            vec![],
            vec![(mk_container(2, "p2", ContainerStatus::Open), 1)],
        );
        let ev2 = diff_snapshots(&prev2, &next2);
        assert_eq!(ev2.len(), 1);
        assert!(matches!(ev2[0], ChangeEvent::PlanAdded { .. }));
    }

    #[test]
    fn milestone_updated_on_field_change() {
        let snap = |title: &str, status: ContainerStatus| DashboardSnapshot {
            issues: vec![],
            plans: vec![],
            milestones: vec![(mk_container(4, title, status), 0)],
            project: "mint".into(),
            milestone_directs: vec![],
        };
        let ev = diff_snapshots(
            &snap("m", ContainerStatus::Open),
            &snap("m2", ContainerStatus::Open),
        );
        assert!(matches!(ev[0], ChangeEvent::MilestoneUpdated { .. }));
        // 无变化不产生事件。
        assert!(
            diff_snapshots(
                &snap("m", ContainerStatus::Open),
                &snap("m", ContainerStatus::Open)
            )
            .is_empty()
        );
    }

    /// plan 字段编辑（title 变化）应产生 PlanUpdated（#334：此前只比 status，字段编辑静默）。
    #[test]
    fn plan_updated_on_field_change() {
        let snap = |title: &str| DashboardSnapshot {
            issues: vec![],
            plans: vec![(mk_container(7, title, ContainerStatus::Open), 0)],
            milestones: vec![],
            project: "mint".into(),
            milestone_directs: vec![],
        };
        let ev = diff_snapshots(&snap("sprint"), &snap("sprint 2"));
        assert!(matches!(ev[0], ChangeEvent::PlanUpdated { .. }));
        // 无变化不产生事件。
        assert!(diff_snapshots(&snap("sprint"), &snap("sprint")).is_empty());
    }

    /// milestone direct 挂载变化产生 MilestoneDirectChanged（#335：attach/detach 不改 milestone 本体字段）。
    #[test]
    fn milestone_direct_change_emits_event() {
        let base = DashboardSnapshot {
            issues: vec![],
            plans: vec![],
            milestones: vec![(mk_container(4, "m", ContainerStatus::Open), 0)],
            project: "mint".into(),
            milestone_directs: vec![],
        };
        let mut attached = base.clone();
        attached.milestone_directs = vec![(4, 100)];
        let ev = diff_snapshots(&base, &attached);
        assert!(
            matches!(
                &ev[0],
                ChangeEvent::MilestoneDirectChanged {
                    milestone_id: 4,
                    count: 1
                }
            ),
            "attach 应产生事件: {:?}",
            ev
        );
        // 无变化不产生事件。
        assert!(diff_snapshots(&base, &base).is_empty());
    }
}
