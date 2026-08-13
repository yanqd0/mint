//! dashboard 状态机测试（model.rs 拆分的独立模块）。

use super::*;
use crate::models::{ContainerStatus, Kind, Status};
use crate::tui::dashboard::diff::DashboardSnapshot;
use rstest::rstest;

/// 无修饰符按键快捷构造（handle_key 改收 TuiKey 后测试用）。
fn k(code: KeyCode) -> TuiKey {
    TuiKey::from_code(code)
}

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
        machine_id: None,
        uid: None,
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
        project: "mint".into(),
        milestone_directs: vec![],
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
        project: "mint".into(),
        milestone_directs: vec![],
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
    assert_eq!(m.selected, 1); // clamp 上界 len（1-indexed）
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
    m.handle_key(k(key));
    assert_eq!(m.selected, sel);
}

#[test]
fn enter_detail_and_esc_back() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![mk_issue(1, Status::Open, None, "1")], vec![]));
    m.selected = 1; // 选中第一个 issue
    m.handle_key(k(KeyCode::Enter));
    assert_eq!(m.view, View::IssueDetail { id: 1 });
    m.handle_key(k(KeyCode::Esc));
    assert_eq!(m.view, View::Issues);
    assert_eq!(m.handle_key(k(KeyCode::Char('q'))), KeyAction::Quit);
}

#[test]
fn ctrl_c_and_q_quit() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![], vec![]));
    assert_eq!(m.handle_key(k(KeyCode::Char('q'))), KeyAction::Quit);
    // Ctrl+C 与 q 等价退出。
    let mut m2 = DashboardModel::new();
    m2.init(snap(vec![], vec![]));
    assert_eq!(
        m2.handle_key(TuiKey {
            code: KeyCode::Char('c'),
            ctrl: true,
            shift: false
        }),
        KeyAction::Quit
    );
    // 纯 'c'（无 ctrl）不退出。
    assert_eq!(m2.handle_key(k(KeyCode::Char('c'))), KeyAction::None);
}

#[test]
fn history_back_forward_chain() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![], vec![]));
    m.navigate(View::Plans);
    m.navigate(View::Milestones);
    assert_eq!(m.view, View::Milestones);
    // Backspace 回退两步。
    m.history_back();
    assert_eq!(m.view, View::Plans);
    m.history_back();
    assert_eq!(m.view, View::Issues);
    // 链首再回退 no-op。
    m.history_back();
    assert_eq!(m.view, View::Issues);
    // Shift+Backspace 前进。
    m.history_forward();
    assert_eq!(m.view, View::Plans);
    m.history_forward();
    assert_eq!(m.view, View::Milestones);
    // 链尾再前进 no-op。
    m.history_forward();
    assert_eq!(m.view, View::Milestones);
}

#[test]
fn history_truncates_forward_on_new_nav() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![], vec![]));
    m.navigate(View::Plans);
    m.navigate(View::Milestones);
    m.history_back(); // 回退到 Plans（链中位）
    assert_eq!(m.view, View::Plans);
    // 中间节点新导航 → 永久截断前进段（Milestones 丢弃）。
    m.navigate(View::Issues);
    assert_eq!(m.view, View::Issues);
    m.history_forward(); // 无前进段
    assert_eq!(m.view, View::Issues);
    m.history_back();
    assert_eq!(m.view, View::Plans);
    m.history_back();
    assert_eq!(m.view, View::Issues);
}

#[test]
fn esc_not_recorded_in_history() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![mk_issue(1, Status::Open, None, "1")], vec![]));
    m.selected = 1;
    m.handle_key(k(KeyCode::Enter)); // Issues → IssueDetail（记录）
    assert_eq!(m.view, View::IssueDetail { id: 1 });
    m.handle_key(k(KeyCode::Esc)); // 回 Issues（switch_tab，不入链）
    assert_eq!(m.view, View::Issues);
    // 历史仍是 [Issues, IssueDetail]，pos 指向 IssueDetail（Esc 不改变链）。
    assert_eq!(m.history.len(), 2);
    assert_eq!(m.history_pos, 1);
}

#[test]
fn backspace_key_navigates_history() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![], vec![]));
    m.handle_key(k(KeyCode::Char('2'))); // → Plans
    m.handle_key(k(KeyCode::Char('3'))); // → Milestones
    m.handle_key(k(KeyCode::Backspace)); // Backspace 回退
    assert_eq!(m.view, View::Plans);
    m.handle_key(TuiKey {
        code: KeyCode::Backspace,
        ctrl: false,
        shift: true,
    }); // Shift+Backspace 前进
    assert_eq!(m.view, View::Milestones);
}

#[test]
fn auto_jump_recorded_in_history() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![], vec![]));
    m.user_idle = 5;
    m.auto_last = 5;
    m.ready.push_back(JumpRequest {
        target: crate::tui::dashboard::types::JumpTarget::Plans,
        flash: vec![],
    });
    m.execute_jump();
    assert_eq!(m.view, View::Plans);
    assert_eq!(m.history.len(), 2); // [Issues, Plans]
    m.history_back();
    assert_eq!(m.view, View::Issues);
}

#[test]
fn reset_history_starts_from_initial_view() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![mk_issue(1, Status::Open, None, "1")], vec![]));
    m.reset_history(View::IssueDetail { id: 1 });
    assert_eq!(m.view, View::IssueDetail { id: 1 });
    assert_eq!(m.history, vec![View::IssueDetail { id: 1 }]);
    assert_eq!(m.history_pos, 0);
    m.history_back(); // 链首 no-op
    assert_eq!(m.view, View::IssueDetail { id: 1 });
}

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
    assert_eq!(m.selected, 0); // 翻页重置选中
    // selected=0 不翻页。
    m.handle_key(k(KeyCode::Char('l')));
    assert_eq!(m.plans_page, 1);
    // 选中 plans 段行再翻上一页。
    m.selected = 1;
    m.handle_key(k(KeyCode::Char('h')));
    assert_eq!(m.plans_page, 0);
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
    assert_eq!(m.selected, 0);
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
    m.handle_key(k(KeyCode::Char('l')));
    assert_eq!(m.page, 1);
    assert_eq!(m.page_issues()[0].id, 3);
    m.handle_key(k(KeyCode::PageDown));
    assert_eq!(m.page, 2);
    assert_eq!(m.page_issues().len(), 1);
    m.handle_key(k(KeyCode::Char('l'))); // 末页无操作
    assert_eq!(m.page, 2);
    m.handle_key(k(KeyCode::PageUp));
    assert_eq!(m.page, 1);
    m.handle_key(k(KeyCode::Char('h')));
    assert_eq!(m.page, 0);
    m.handle_key(k(KeyCode::Char('h'))); // 首页无操作
    assert_eq!(m.page, 0);
}

/// list --tui 容器视图：all=false 排除 done（与 TSV 容器 list 默认只显活跃一致）。
#[test]
fn visible_containers_filter_out_done_when_all_false() {
    let mut m = DashboardModel::new();
    m.init(snap_full(
        vec![],
        vec![(mk_plan(7, None, "1"), 0)],
        vec![(mk_container(4), 0), (mk_container(5), 0)],
    ));
    m.milestones[0].0.status = ContainerStatus::Done; // milestone 4 置 done
    m.filter = Some(crate::tui::dashboard::types::IssueFilter {
        all: false,
        status: None,
        label: None,
        priority: None,
    });
    m.view = View::Milestones;
    let ms = m.visible_milestones();
    assert_eq!(ms.len(), 1, "all=false 应排除 done milestone");
    assert_eq!(ms[0].0.id, 5);
    // all=true 不过滤
    m.filter.as_mut().unwrap().all = true;
    assert_eq!(m.visible_milestones().len(), 2);
    // plans 同样过滤 done
    m.plans[0].0.status = ContainerStatus::Done;
    m.filter.as_mut().unwrap().all = false;
    m.view = View::Plans;
    assert_eq!(m.visible_plans().len(), 0, "done plan 应被排除");
    m.filter.as_mut().unwrap().all = true;
    assert_eq!(m.visible_plans().len(), 1);
}
