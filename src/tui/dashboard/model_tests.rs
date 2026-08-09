//! dashboard 状态机测试（model.rs 拆分的独立模块）。

use super::*;
use crate::models::{ContainerStatus, Kind, Status};
use crate::tui::dashboard::diff::DashboardSnapshot;
use rstest::rstest;

fn mk_issue(id: i64, status: Status, plan_id: Option<i64>, updated: &str) -> Issue {
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
        updated_at: updated.into(),
    }
}

fn mk_container(id: i64) -> Container {
    Container {
        id,
        title: "p".into(),
        version: None,
        body: None,
        milestone_id: None,
        status: ContainerStatus::Open,
        created_at: "t".into(),
        updated_at: "t".into(),
    }
}

fn mk_plan(id: i64, milestone: Option<i64>, updated: &str) -> Container {
    Container {
        id,
        title: "p".into(),
        version: None,
        body: None,
        milestone_id: milestone,
        status: ContainerStatus::Open,
        created_at: "t".into(),
        updated_at: updated.into(),
    }
}

fn snap(issues: Vec<Issue>, plans: Vec<(Container, i64)>) -> DashboardSnapshot {
    DashboardSnapshot {
        issues,
        plans,
        milestones: vec![],
    }
}

fn snap_full(
    issues: Vec<Issue>,
    plans: Vec<(Container, i64)>,
    milestones: Vec<(Container, i64)>,
) -> DashboardSnapshot {
    DashboardSnapshot {
        issues,
        plans,
        milestones,
    }
}

#[test]
fn baseline_sorted_and_selection_kept() {
    let mut m = DashboardModel::new();
    m.init(snap(
        vec![
            mk_issue(1, Status::Open, None, "10:00"),
            mk_issue(2, Status::Dev, None, "12:00"),
        ],
        vec![],
    ));
    assert_eq!(m.view, View::Issues);
    assert_eq!(m.feed[0].issue().unwrap().id, 2); // 最新在前
    let r = m.refresh(&snap(
        vec![
            mk_issue(1, Status::Open, None, "10:00"),
            mk_issue(2, Status::Dev, None, "12:00"),
            mk_issue(3, Status::Open, None, "13:00"),
        ],
        vec![],
    ));
    assert_eq!(r.new_events, 1);
    assert_eq!(m.selected, 0); // 保持
    assert_eq!(m.feed[0].issue().unwrap().id, 3);
}

#[test]
fn selection_clamped_when_list_shrinks() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![mk_issue(1, Status::Open, None, "1")], vec![]));
    m.selected = 5;
    m.refresh(&snap(vec![mk_issue(1, Status::Open, None, "1")], vec![]));
    assert_eq!(m.selected, 0);
}

#[rstest]
#[case(KeyCode::Char('j'), 1)]
#[case(KeyCode::Down, 1)]
#[case(KeyCode::Char('k'), 0)]
#[case(KeyCode::Up, 0)]
fn navigation_keys(#[case] key: KeyCode, #[case] sel: usize) {
    let mut m = DashboardModel::new();
    m.init(snap(
        vec![
            mk_issue(1, Status::Open, None, "1"),
            mk_issue(2, Status::Open, None, "2"),
        ],
        vec![],
    ));
    m.handle_key(key);
    assert_eq!(m.selected, sel);
}

#[test]
fn enter_detail_and_esc_back() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![mk_issue(1, Status::Open, None, "1")], vec![]));
    m.handle_key(KeyCode::Enter);
    assert_eq!(m.view, View::IssueDetail { id: 1 });
    m.handle_key(KeyCode::Esc);
    assert_eq!(m.view, View::Issues);
    assert!(m.handle_key(KeyCode::Char('q')));
}

#[test]
fn visible_issues_filters_by_plan() {
    let mut m = DashboardModel::new();
    m.init(snap(
        vec![
            mk_issue(1, Status::Dev, Some(7), "1"),
            mk_issue(2, Status::Open, None, "2"),
        ],
        vec![],
    ));
    assert_eq!(m.visible_issues().len(), 2);
    m.view = View::PlanDetail { plan_id: 7 };
    let v = m.visible_issues();
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].id, 1);
}

#[test]
fn number_keys_and_tab_switch_tabs() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![], vec![]));
    m.handle_key(KeyCode::Char('2'));
    assert_eq!(m.view, View::Plans);
    m.handle_key(KeyCode::Char('3'));
    assert_eq!(m.view, View::Milestones);
    m.handle_key(KeyCode::Char('1'));
    assert_eq!(m.view, View::Issues);
    m.handle_key(KeyCode::Tab);
    assert_eq!(m.view, View::Plans);
    m.handle_key(KeyCode::Tab);
    assert_eq!(m.view, View::Milestones);
    m.handle_key(KeyCode::Tab);
    assert_eq!(m.view, View::Issues);
}

#[test]
fn plans_tab_enter_opens_plan_detail() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![], vec![(mk_plan(7, Some(4), "1"), 0)]));
    m.handle_key(KeyCode::Char('2'));
    m.handle_key(KeyCode::Enter);
    assert_eq!(m.view, View::PlanDetail { plan_id: 7 });
    m.handle_key(KeyCode::Esc);
    assert_eq!(m.view, View::Plans);
}

#[test]
fn p_key_jumps_to_plan_detail_from_issue() {
    let mut m = DashboardModel::new();
    m.init(snap(
        vec![mk_issue(1, Status::Dev, Some(7), "1")],
        vec![(mk_plan(7, Some(4), "1"), 0)],
    ));
    m.handle_key(KeyCode::Char('p'));
    assert_eq!(m.view, View::PlanDetail { plan_id: 7 });
}

#[test]
fn milestone_plans_filters_and_sorts() {
    let mut m = DashboardModel::new();
    m.init(snap(
        vec![],
        vec![
            (mk_plan(7, Some(4), "10:00"), 0),
            (mk_plan(8, Some(4), "12:00"), 0),
            (mk_plan(9, None, "11:00"), 0),
        ],
    ));
    let ps = m.milestone_plans(4);
    assert_eq!(ps.len(), 2);
    assert_eq!(ps[0].0.id, 8); // updated_at 最新在前
    assert_eq!(ps[1].0.id, 7);
}

#[test]
fn plan_progress_counts_done_over_total() {
    let mut m = DashboardModel::new();
    m.init(snap(
        vec![
            mk_issue(1, Status::Done, Some(7), "1"),
            mk_issue(2, Status::Open, Some(7), "2"),
            mk_issue(3, Status::Dev, Some(7), "3"),
            mk_issue(4, Status::Open, None, "4"),
        ],
        vec![],
    ));
    assert_eq!(m.plan_progress(7), (1, 3));
    assert_eq!(m.plan_progress(8), (0, 0));
}

#[test]
fn milestones_tab_enter_opens_milestone_detail() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![], vec![]));
    m.milestones = vec![(mk_container(4), 0)];
    m.handle_key(KeyCode::Char('3'));
    m.handle_key(KeyCode::Enter);
    assert_eq!(m.view, View::MilestoneDetail { milestone_id: 4 });
    m.handle_key(KeyCode::Esc);
    assert_eq!(m.view, View::Milestones);
}

#[test]
fn milestone_detail_enter_opens_plan_detail() {
    let mut m = DashboardModel::new();
    m.init(snap_full(
        vec![mk_issue(1, Status::Dev, Some(7), "1")],
        vec![(mk_plan(7, Some(4), "1"), 0)],
        vec![(mk_container(4), 0)],
    ));
    m.view = View::MilestoneDetail { milestone_id: 4 };
    m.handle_key(KeyCode::Enter);
    assert_eq!(m.view, View::PlanDetail { plan_id: 7 });
    m.handle_key(KeyCode::Esc);
    assert_eq!(m.view, View::Plans);
}

#[test]
fn number_keys_from_detail_switch_tab() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![mk_issue(1, Status::Open, None, "1")], vec![]));
    m.handle_key(KeyCode::Enter); // IssueDetail
    assert_eq!(m.view, View::IssueDetail { id: 1 });
    m.handle_key(KeyCode::Char('2'));
    assert_eq!(m.view, View::Plans);
}
