//! dashboard 状态机测试（detail）。

use super::*;
use crossterm::event::KeyCode;

#[test]
fn milestone_detail_paging_routes_by_cursor() {
    let plans: Vec<(Container, i64)> = (1..=12).map(|i| (mk_plan(i, Some(4), "1"), 0)).collect();
    let mut m = DashboardModel::new();
    m.init(snap_full(vec![], plans, vec![(mk_container(4), 0)]));
    m.view = View::MilestoneDetail { milestone_id: 4 };
    m.selected = 1; // plans 段第 1 行
    m.handle_key(k(KeyCode::Char('l'))); // 翻 plans 页
    assert_eq!(m.plans_page, 1);
    assert_eq!(m.issues_page, 0);
    assert_eq!(m.selected, 1); // 翻页保持相对行（页 1 仅 2 行仍第 1 行）
    // selected=0 不翻页。
    m.selected = 0;
    m.handle_key(k(KeyCode::Char('l')));
    assert_eq!(m.plans_page, 1);
    // 选中 plans 段行再翻上一页。
    m.selected = 1;
    m.handle_key(k(KeyCode::Char('h')));
    assert_eq!(m.plans_page, 0);
    assert_eq!(m.selected, 1); // 翻回保持相对行
}

#[test]
fn milestone_detail_paging_routes_issues_segment() {
    let issues: Vec<Issue> = (1..=12)
        .map(|i| mk_issue(i, Status::Open, None, "1"))
        .collect();
    let mut m = DashboardModel::new();
    m.init(snap_full(
        issues,
        vec![(mk_plan(7, Some(4), "1"), 0)],
        vec![(mk_container(4), 0)],
    ));
    m.milestone_directs = (1..=12).map(|i| (4, i)).collect();
    m.view = View::MilestoneDetail { milestone_id: 4 };
    m.selected = 1; // plans 段（仅 1 个 plan，1 页）
    m.handle_key(k(KeyCode::Char('l'))); // plans 仅 1 页不翻
    assert_eq!(m.plans_page, 0);
    m.selected = 2; // issues 段第 1 行（plans 段 1..=1）
    m.handle_key(k(KeyCode::Char('l'))); // 翻 issues 页
    assert_eq!(m.issues_page, 1);
    assert_eq!(m.plans_page, 0);
    assert_eq!(m.selected, 2); // 翻页保持相对行
}

#[test]
fn milestone_detail_enter_uses_current_page_plan() {
    let plans: Vec<(Container, i64)> = (1..=12).map(|i| (mk_plan(i, Some(4), "1"), 0)).collect();
    let mut m = DashboardModel::new();
    m.init(snap_full(vec![], plans, vec![(mk_container(4), 0)]));
    m.view = View::MilestoneDetail { milestone_id: 4 };
    m.plans_page = 1; // 第 2 页：plans 11..12
    m.selected = 1;
    m.handle_key(k(KeyCode::Enter));
    assert_eq!(m.view, View::PlanDetail { plan_id: 11 });
}

#[test]
fn plan_detail_enter_opens_issue_detail() {
    let mut m = DashboardModel::new();
    m.init(snap(
        vec![
            mk_issue(1, Status::Dev, Some(7), "1"),
            mk_issue(2, Status::Open, None, "2"),
        ],
        vec![],
    ));
    m.view = View::PlanDetail { plan_id: 7 };
    m.selected = 1; // 选中 plan 7 的第一个 issue
    m.handle_key(k(KeyCode::Enter));
    assert_eq!(m.view, View::IssueDetail { id: 1 });
}

#[test]
fn issue_detail_p_and_m_navigate() {
    let mut m = DashboardModel::new();
    m.init(snap_full(
        vec![mk_issue(1, Status::Dev, Some(7), "1")],
        vec![(mk_plan(7, Some(4), "1"), 0)],
        vec![(mk_container(4), 0)],
    ));
    m.view = View::IssueDetail { id: 1 };
    m.handle_key(k(KeyCode::Char('p')));
    assert_eq!(m.view, View::PlanDetail { plan_id: 7 });
    m.view = View::IssueDetail { id: 1 };
    m.handle_key(k(KeyCode::Char('m')));
    assert_eq!(m.view, View::MilestoneDetail { milestone_id: 4 });
}

#[test]
fn plan_detail_m_navigates_to_milestone() {
    let mut m = DashboardModel::new();
    m.init(snap_full(
        vec![mk_issue(1, Status::Dev, Some(7), "1")],
        vec![(mk_plan(7, Some(4), "1"), 0)],
        vec![(mk_container(4), 0)],
    ));
    m.view = View::PlanDetail { plan_id: 7 };
    m.selected = 0; // 无选中也应跳（PlanDetail 分支与选中行无关）
    m.handle_key(k(KeyCode::Char('m')));
    assert_eq!(m.view, View::MilestoneDetail { milestone_id: 4 });
}

#[test]
fn plan_groups_skip_empty_milestones() {
    let mut m = DashboardModel::new();
    m.init(snap_full(
        vec![],
        vec![(mk_plan(7, Some(4), "1"), 0)],
        vec![(mk_container(4), 0), (mk_container(5), 0)],
    ));
    let groups = m.plan_groups();
    // 有 plan 的 ms4 产生组；空 ms5（无 plan）被跳过 → 仅 1 组，避免孤行标题。
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].plans.len(), 1);
}

#[test]
fn plans_paging_covers_all_plans_across_groups() {
    // free plan + ms4 组 2 plans + ms3 组 2 plans（组标题计入可见行）。
    let mut m = DashboardModel::new();
    m.init(snap_full(
        vec![],
        vec![
            (mk_plan(1, Some(4), "1"), 0),
            (mk_plan(2, Some(4), "2"), 0),
            (mk_plan(3, Some(3), "1"), 0),
            (mk_plan(4, Some(3), "2"), 0),
            (mk_plan(5, None, "1"), 0),
        ],
        vec![(mk_container(4), 0), (mk_container(3), 0)],
    ));
    m.view = View::Plans;
    m.page_size = 3;
    // 扁平列表（无组标题）5 plans → ceil(5/3)=2 页。
    assert_eq!(m.pages(), 2);
    let mut seen: Vec<i64> = Vec::new();
    for page in 0..2 {
        m.page = page;
        seen.extend(m.page_plans().iter().map(|(c, _)| c.id));
    }
    seen.sort();
    assert_eq!(
        seen,
        vec![1, 2, 3, 4, 5],
        "跨页应覆盖全部 plan 无丢失: {seen:?}"
    );
}

#[test]
fn milestone_scope_direct_issues_first() {
    // ms4：直属 issue 2 + plan 7 的间接 issue 1。
    let mut m = DashboardModel::new();
    m.init(snap_full(
        vec![
            mk_issue(1, Status::Open, Some(7), "1"),
            mk_issue(2, Status::Done, None, "1"),
        ],
        vec![(mk_plan(7, Some(4), "1"), 0)],
        vec![(mk_container(4), 0)],
    ));
    m.milestone_directs = vec![(4, 2)];
    m.view = View::MilestoneDetail { milestone_id: 4 };
    let v = m.scope_issues();
    assert_eq!(v.len(), 2);
    assert_eq!(v[0].id, 2, "直属 issue 应在最前");
    assert_eq!(v[1].id, 1, "间接 issue 随后");
}
