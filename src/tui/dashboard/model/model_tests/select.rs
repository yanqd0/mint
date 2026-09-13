//! dashboard 状态机测试（select）。

use super::*;

/// prune_detail：PlanDetail/MilestoneDetail 实体失效 → 回对应 tab；实体仍在不切换。
#[test]
fn prune_detail_plan_and_milestone_views() {
    // PlanDetail 失效 → Plans。
    let mut m = DashboardModel::new();
    m.init(snap_full(vec![], vec![(mk_plan(7, None, "1"), 0)], vec![]));
    m.view = View::PlanDetail { plan_id: 7 };
    m.plans.clear();
    m.prune_detail();
    assert_eq!(m.view, View::Plans);
    // MilestoneDetail 失效 → Milestones。
    let mut m = DashboardModel::new();
    m.init(snap_full(vec![], vec![], vec![(mk_container(4), 0)]));
    m.view = View::MilestoneDetail { milestone_id: 4 };
    m.milestones.clear();
    m.prune_detail();
    assert_eq!(m.view, View::Milestones);
    // 实体仍在 → 不切换。
    let mut m = DashboardModel::new();
    m.init(snap_full(vec![], vec![(mk_plan(7, None, "1"), 0)], vec![]));
    m.view = View::PlanDetail { plan_id: 7 };
    m.prune_detail();
    assert_eq!(m.view, View::PlanDetail { plan_id: 7 });
}

/// selected_plan_id：Issues 行 plan_id / Plans 行 plan id / IssueDetail 当前 issue。
/// 注意 selected 从 1 起（0 = 无选中，selected_idx() 返回 None）。
#[test]
fn selected_plan_id_resolves_by_view() {
    // Issues 行带 plan_id。
    let mut m = DashboardModel::new();
    m.init(snap(vec![mk_issue(1, Status::Dev, Some(7), "1")], vec![]));
    m.selected = 1;
    assert_eq!(m.selected_plan_id(), Some(7));
    // Issues 行无 plan → None。
    let mut m = DashboardModel::new();
    m.init(snap(vec![mk_issue(1, Status::Dev, None, "1")], vec![]));
    m.selected = 1;
    assert_eq!(m.selected_plan_id(), None);
    // Plans 行 → plan id。
    let mut m = DashboardModel::new();
    m.init(snap_full(vec![], vec![(mk_plan(7, None, "1"), 0)], vec![]));
    m.view = View::Plans;
    m.selected = 1;
    assert_eq!(m.selected_plan_id(), Some(7));
    // IssueDetail → 当前 issue 的 plan_id（不依赖 selected）。
    let mut m = DashboardModel::new();
    m.init(snap(vec![mk_issue(1, Status::Dev, Some(7), "1")], vec![]));
    m.view = View::IssueDetail { id: 1 };
    assert_eq!(m.selected_plan_id(), Some(7));
}

/// selected_milestone_id：Issues 行经 plan / Plans 行 / Milestones 行 / IssueDetail。
#[test]
fn selected_milestone_id_resolves_by_view() {
    // Issues 行 → issue.plan → plan.milestone。
    let mut m = DashboardModel::new();
    m.init(snap_full(
        vec![mk_issue(1, Status::Dev, Some(7), "1")],
        vec![(mk_plan(7, Some(4), "1"), 0)],
        vec![],
    ));
    m.selected = 1;
    assert_eq!(m.selected_milestone_id(), Some(4));
    // Plans 行 → plan.milestone。
    let mut m = DashboardModel::new();
    m.init(snap_full(
        vec![],
        vec![(mk_plan(7, Some(4), "1"), 0)],
        vec![],
    ));
    m.view = View::Plans;
    m.selected = 1;
    assert_eq!(m.selected_milestone_id(), Some(4));
    // Milestones 行 → milestone id。
    let mut m = DashboardModel::new();
    m.init(snap_full(vec![], vec![], vec![(mk_container(4), 0)]));
    m.view = View::Milestones;
    m.selected = 1;
    assert_eq!(m.selected_milestone_id(), Some(4));
    // IssueDetail → 当前 issue 的 plan 的 milestone（不依赖 selected）。
    let mut m = DashboardModel::new();
    m.init(snap_full(
        vec![mk_issue(1, Status::Dev, Some(7), "1")],
        vec![(mk_plan(7, Some(4), "1"), 0)],
        vec![],
    ));
    m.view = View::IssueDetail { id: 1 };
    assert_eq!(m.selected_milestone_id(), Some(4));
}
