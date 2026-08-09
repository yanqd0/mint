//! dashboard 状态机行为测试（自动切换 + 分页等，model_tests.rs 拆分）。

use super::*;
use crate::models::{ContainerStatus, Kind, Status};
use crate::tui::dashboard::diff::DashboardSnapshot;

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

/// 让自动切换前置满足：用户空闲 ≥5s 且距上次自动切换 ≥5s。
fn enable_auto(m: &mut DashboardModel) {
    m.user_idle = 5;
    m.auto_last = 5;
}

#[test]
fn auto_switch_requires_idle_gap() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![], vec![]));
    // 用户空闲不足（刚 init）→ 有 dev issue 也不自动切
    let r = m.refresh(&snap(
        vec![mk_issue(1, Status::Dev, Some(7), "11:00")],
        vec![(mk_container(7), 1)],
    ));
    assert_eq!(r.auto_plan, None);
    assert_eq!(m.view, View::Issues);
}

#[test]
fn auto_switches_to_plan_detail_and_back_to_plans() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![], vec![]));
    assert_eq!(m.view, View::Issues);
    enable_auto(&mut m);
    let r = m.refresh(&snap(
        vec![mk_issue(1, Status::Dev, Some(7), "11:00")],
        vec![(mk_container(7), 1)],
    ));
    assert_eq!(r.auto_plan, Some(7));
    assert_eq!(m.view, View::PlanDetail { plan_id: 7 });
    // 执行结束（全 done）→ 间隔 ≥5 后回 Plans tab
    let end = snap(
        vec![mk_issue(1, Status::Done, Some(7), "12:00")],
        vec![(mk_container(7), 1)],
    );
    for _ in 0..5 {
        m.refresh(&end);
    }
    assert_eq!(m.view, View::Plans);
}

#[test]
fn user_esc_prevents_reclaiming_same_plan() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![], vec![]));
    enable_auto(&mut m);
    m.refresh(&snap(
        vec![mk_issue(1, Status::Dev, Some(7), "11:00")],
        vec![(mk_container(7), 1)],
    ));
    assert_eq!(m.view, View::PlanDetail { plan_id: 7 });
    m.handle_key(KeyCode::Esc);
    assert_eq!(m.view, View::Plans);
    enable_auto(&mut m);
    let r = m.refresh(&snap(
        vec![mk_issue(1, Status::Dev, Some(7), "11:30")],
        vec![(mk_container(7), 1)],
    ));
    assert_eq!(r.auto_plan, None); // last_auto=7 → 不抢占
    assert_eq!(m.view, View::Plans);
}

#[test]
fn milestone_detail_hold_expires_back_to_plans() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![], vec![]));
    m.milestones = vec![(mk_container(4), 0)];
    m.view = View::MilestoneDetail { milestone_id: 4 };
    m.milestone_hold = Some(2);
    enable_auto(&mut m);
    let s4 = snap_full(vec![], vec![], vec![(mk_container(4), 0)]);
    m.refresh(&s4); // 2 → 1，仍显示
    assert_eq!(m.view, View::MilestoneDetail { milestone_id: 4 });
    m.refresh(&s4); // 1 → 0 → 回 Plans tab
    assert_eq!(m.view, View::Plans);
    assert_eq!(m.milestone_hold, None);
}

#[test]
fn plan_end_shows_milestone_detail_then_plans() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![], vec![(mk_plan(7, Some(4), "1"), 0)]));
    m.milestones = vec![(mk_container(4), 0)];
    enable_auto(&mut m);
    let m4 = vec![(mk_container(4), 0)];
    // plan 执行中 → 自动切 plan 详情
    m.refresh(&snap_full(
        vec![mk_issue(1, Status::Dev, Some(7), "11:00")],
        vec![(mk_plan(7, Some(4), "1"), 0)],
        m4.clone(),
    ));
    assert_eq!(m.view, View::PlanDetail { plan_id: 7 });
    let end = snap_full(
        vec![mk_issue(1, Status::Done, Some(7), "12:00")],
        vec![(mk_plan(7, Some(4), "1"), 0)],
        m4.clone(),
    );
    // 间隔 ≥5 tick 后 plan 结束 → 切所属 milestone 详情，hold 启动
    for _ in 0..5 {
        m.refresh(&end);
    }
    assert_eq!(m.view, View::MilestoneDetail { milestone_id: 4 });
    assert!(m.milestone_hold.is_some());
    // 倒计时归零 + 间隔 → 回 Plans tab
    for _ in 0..8 {
        m.refresh(&end);
    }
    assert_eq!(m.view, View::Plans);
    assert_eq!(m.milestone_hold, None);
}

#[test]
fn user_interaction_cancels_milestone_hold() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![], vec![(mk_plan(7, Some(4), "1"), 0)]));
    m.milestones = vec![(mk_container(4), 0)];
    enable_auto(&mut m);
    let m4 = vec![(mk_container(4), 0)];
    m.refresh(&snap_full(
        vec![mk_issue(1, Status::Dev, Some(7), "11:00")],
        vec![(mk_plan(7, Some(4), "1"), 0)],
        m4.clone(),
    ));
    let end = snap_full(
        vec![mk_issue(1, Status::Done, Some(7), "12:00")],
        vec![(mk_plan(7, Some(4), "1"), 0)],
        m4.clone(),
    );
    for _ in 0..5 {
        m.refresh(&end);
    }
    assert!(matches!(m.view, View::MilestoneDetail { .. }));
    assert!(m.milestone_hold.is_some());
    // 用户按键 → 接管，取消自动倒计时
    m.handle_key(KeyCode::Char('j'));
    assert_eq!(m.milestone_hold, None);
    // 后续 refresh 不再自动回
    for _ in 0..6 {
        m.refresh(&end);
    }
    assert!(matches!(m.view, View::MilestoneDetail { .. }));
}

#[test]
fn pagination_with_page_size() {
    let mut m = DashboardModel::new();
    m.page_size = 2;
    m.init(snap(
        vec![
            mk_issue(1, Status::Open, None, "1"),
            mk_issue(2, Status::Open, None, "2"),
            mk_issue(3, Status::Open, None, "3"),
            mk_issue(4, Status::Open, None, "4"),
            mk_issue(5, Status::Open, None, "5"),
        ],
        vec![],
    ));
    // page_size 2 → 3 页（updated 倒序：5,4 | 3,2 | 1）
    assert_eq!(m.pages(), 3);
    assert_eq!(m.page_issues().len(), 2);
    assert_eq!(m.page_issues()[0].id, 5);
    m.handle_key(KeyCode::Char('l'));
    assert_eq!(m.page, 1);
    assert_eq!(m.page_issues()[0].id, 3);
    m.handle_key(KeyCode::PageDown);
    assert_eq!(m.page, 2);
    assert_eq!(m.page_issues().len(), 1);
    m.handle_key(KeyCode::Char('l')); // 末页无操作
    assert_eq!(m.page, 2);
    m.handle_key(KeyCode::PageUp);
    assert_eq!(m.page, 1);
    m.handle_key(KeyCode::Char('h'));
    assert_eq!(m.page, 0);
    m.handle_key(KeyCode::Char('h')); // 首页无操作
    assert_eq!(m.page, 0);
}
