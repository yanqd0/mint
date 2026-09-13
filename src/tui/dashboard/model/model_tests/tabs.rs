//! dashboard 状态机测试（tabs）。

use super::*;
use crossterm::event::KeyCode;

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
    m.handle_key(k(KeyCode::Char('2')));
    assert_eq!(m.view, View::Plans);
    m.handle_key(k(KeyCode::Char('3')));
    assert_eq!(m.view, View::Milestones);
    m.handle_key(k(KeyCode::Char('1')));
    assert_eq!(m.view, View::Issues);
    m.handle_key(k(KeyCode::Tab));
    assert_eq!(m.view, View::Plans);
    m.handle_key(k(KeyCode::Tab));
    assert_eq!(m.view, View::Milestones);
    m.handle_key(k(KeyCode::Tab));
    assert_eq!(m.view, View::Issues);
}

#[test]
fn plans_tab_enter_opens_plan_detail() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![], vec![(mk_plan(7, Some(4), "1"), 0)]));
    m.handle_key(k(KeyCode::Char('2')));
    m.selected = 1; // 选中第一个 plan（selected 1-indexed，0=无选中）
    m.handle_key(k(KeyCode::Enter));
    assert_eq!(m.view, View::PlanDetail { plan_id: 7 });
    m.handle_key(k(KeyCode::Esc));
    assert_eq!(m.view, View::Plans);
}

#[test]
fn p_key_jumps_to_plan_detail_from_issue() {
    let mut m = DashboardModel::new();
    m.init(snap(
        vec![mk_issue(1, Status::Dev, Some(7), "1")],
        vec![(mk_plan(7, Some(4), "1"), 0)],
    ));
    m.selected = 1; // 选中 issue（0=无选中）
    m.handle_key(k(KeyCode::Char('p')));
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
    m.handle_key(k(KeyCode::Char('3')));
    m.selected = 1; // 选中第一个 milestone
    m.handle_key(k(KeyCode::Enter));
    assert_eq!(m.view, View::MilestoneDetail { milestone_id: 4 });
    m.handle_key(k(KeyCode::Esc));
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
    m.selected = 1; // plans 段第一个 plan
    m.handle_key(k(KeyCode::Enter));
    assert_eq!(m.view, View::PlanDetail { plan_id: 7 });
    m.handle_key(k(KeyCode::Esc));
    assert_eq!(m.view, View::Plans);
}

#[test]
fn number_keys_from_detail_switch_tab() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![mk_issue(1, Status::Open, None, "1")], vec![]));
    m.selected = 1; // 选中第一个 issue
    m.handle_key(k(KeyCode::Enter)); // IssueDetail
    assert_eq!(m.view, View::IssueDetail { id: 1 });
    m.handle_key(k(KeyCode::Char('2')));
    assert_eq!(m.view, View::Plans);
}
