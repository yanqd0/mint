//! dashboard 状态机测试（paging）。

use super::*;
use crate::models::ContainerStatus;
use crossterm::event::KeyCode;

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
    m.selected = 2; // 第 2 行
    m.handle_key(k(KeyCode::Char('l')));
    assert_eq!(m.page, 1);
    assert_eq!(m.page_issues()[0].id, 3);
    assert_eq!(m.selected, 2); // 翻页保持相对行
    m.handle_key(k(KeyCode::PageDown));
    assert_eq!(m.page, 2);
    assert_eq!(m.page_issues().len(), 1);
    assert_eq!(m.selected, 1); // 末页仅 1 行，夹到 1
    m.handle_key(k(KeyCode::Char('l'))); // 末页无操作
    assert_eq!(m.page, 2);
    m.handle_key(k(KeyCode::PageUp));
    assert_eq!(m.page, 1);
    assert_eq!(m.selected, 1); // 翻回保持
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

/// #241：普通分页翻页保持相对行；短末页夹到新页长；翻回保持。
#[test]
fn pagination_keeps_relative_row_and_clamps_short_last_page() {
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
    m.selected = 2; // 页 0 第 2 行（id 4）
    m.handle_key(k(KeyCode::PageDown)); // → 页 1（3,2），保持相对行
    assert_eq!(m.page, 1);
    assert_eq!(m.selected, 2);
    m.handle_key(k(KeyCode::PageDown)); // → 页 2（1，仅 1 行），夹到 1
    assert_eq!(m.page, 2);
    assert_eq!(m.selected, 1);
    m.handle_key(k(KeyCode::PageUp)); // → 页 1，保持 1
    assert_eq!(m.page, 1);
    assert_eq!(m.selected, 1);
}

/// #241：无选中（selected=0）翻页保持无选中（对齐 j/k 语义）。
#[test]
fn pagination_without_selection_stays_unselected() {
    let mut m = DashboardModel::new();
    m.page_size = 2;
    m.init(snap(
        vec![
            mk_issue(1, Status::Open, None, "1"),
            mk_issue(2, Status::Open, None, "2"),
            mk_issue(3, Status::Open, None, "3"),
        ],
        vec![],
    ));
    assert_eq!(m.selected, 0);
    m.handle_key(k(KeyCode::Char('l')));
    assert_eq!(m.page, 1);
    assert_eq!(m.selected, 0, "无选中翻页保持无选中");
}

/// #242 核心防回归：plans 段末行翻到短页，按段夹取不流入 issues 段。
#[test]
fn milestone_detail_plans_segment_clamps_on_shorter_new_page() {
    // 12 plans（页 0 10 行，页 1 2 行）+ 5 直属 issues（np=2 时 issues 段非空）。
    let plans: Vec<(Container, i64)> = (1..=12).map(|i| (mk_plan(i, Some(4), "1"), 0)).collect();
    let issues: Vec<Issue> = (1..=5)
        .map(|i| mk_issue(i, Status::Open, None, "1"))
        .collect();
    let mut m = DashboardModel::new();
    m.init(snap_full(
        issues.clone(),
        plans.clone(),
        vec![(mk_container(4), 0)],
    ));
    m.milestone_directs = (1..=5).map(|i| (4, i)).collect();
    m.view = View::MilestoneDetail { milestone_id: 4 };
    m.plans_page_size = 10;
    m.issues_page_size = 5;
    m.selected = 10; // plans 段页 0 末行
    m.handle_key(k(KeyCode::Char('l'))); // 翻 plans 页 → 页 1（2 行）
    assert_eq!(m.plans_page, 1);
    assert_eq!(m.selected, 2, "plans 段按新页长夹取，不流入 issues 段");
    m.handle_key(k(KeyCode::Enter));
    assert_eq!(
        m.view,
        View::PlanDetail { plan_id: 12 },
        "仍停 plans 段，Enter 进 plan"
    );
}

/// #242：issues 段翻页保持相对行；翻回保持。
#[test]
fn milestone_detail_issues_segment_keeps_relative_row() {
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
    m.issues_page_size = 10;
    m.selected = 3; // issues 段第 2 行（np=1）
    m.handle_key(k(KeyCode::Char('l'))); // 翻 issues 页 → 页 1（2 行）
    assert_eq!(m.issues_page, 1);
    assert_eq!(m.plans_page, 0);
    assert_eq!(m.selected, 3, "issues 段翻页保持相对行（恒 > np）");
    m.handle_key(k(KeyCode::Char('h'))); // 翻回页 0
    assert_eq!(m.issues_page, 0);
    assert_eq!(m.selected, 3, "翻回保持");
}

// ── #243 光标记忆 ─────────────────────────────────────────────
