//! diff.rs 拆分的独立测试模块。

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
        project: Some("mint".into()),
        test_cmd: None,
        dropped_reason: None,
        last_commit_id: None,
        plan_id: None,
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
