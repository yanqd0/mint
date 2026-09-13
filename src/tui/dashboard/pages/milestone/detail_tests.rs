//! detail.rs 拆分的独立测试模块。

use super::*;
use crate::models::Container;
use crate::tui::dashboard::pages::tests_common::{
    buffer_text, mk_container, mk_issue, model_full, test_backend,
};
use crate::tui::dashboard::types::View;

#[test]
fn milestone_detail_shows_basic_plans_and_direct_issues() {
    let mut m = model_full(
        vec![
            mk_issue(1, "open one", Status::Open, Some(7)),
            mk_issue(2, "done one", Status::Done, Some(7)),
        ],
        vec![(mk_container(7, "tui plan", None, Some(4)), 0)],
        vec![(mk_container(4, "TUI", Some("0.4.0"), None), 0)],
    );
    m.milestone_directs = vec![(4, 1)]; // issue 1 直属 milestone 4
    m.view = View::MilestoneDetail { milestone_id: 4 };
    let mut terminal = test_backend(100, 20);
    terminal
        .draw(|f| draw_detail(f, &mut m, 4, f.area()))
        .unwrap();
    let text = buffer_text(terminal.backend().buffer()).join("\n");
    assert!(text.contains("#4 TUI"), "标题: {text}");
    assert!(text.contains("0.4.0"), "version: {text}");
    assert!(text.contains("plans"), "plan 列表标题: {text}");
    assert!(text.contains("tui plan"), "plan 行: {text}");
    assert!(text.contains("╭─issues"), "issues panel: {text}");
    assert!(text.contains("open one"), "直属 issue 行: {text}");
}

#[test]
fn progress_panel_shows_aggregate_direct_and_indirect() {
    // plan 7 属 ms4（1 个 done 间接）+ issue 2 直属 ms4（dropped）。
    let mut m = model_full(
        vec![
            mk_issue(1, "done in plan", Status::Done, Some(7)),
            mk_issue(2, "direct dropped", Status::Dropped, None),
        ],
        vec![(mk_container(7, "tui plan", None, Some(4)), 0)],
        vec![(mk_container(4, "TUI", Some("0.4.0"), None), 0)],
    );
    m.milestone_directs = vec![(4, 2)];
    m.view = View::MilestoneDetail { milestone_id: 4 };
    let mut terminal = test_backend(80, 20);
    terminal
        .draw(|f| draw_detail(f, &mut m, 4, f.area()))
        .unwrap();
    let text = buffer_text(terminal.backend().buffer()).join("\n");
    // 直接+间接 2 issue（done + dropped）→ 分组行含 dropped 50%。
    assert!(text.contains("dropped 50%"), "聚合进度: {text}");
}

#[test]
fn progress_panel_min_one_percent_for_present_group() {
    // 100 done（经 plan 间接）+ 1 working（直属）：working 占比 <1%，面板百分比仍显 1%
    // （与进度条 min-1 可见性一致，不再 working 0%）。
    let mut issues: Vec<Issue> = (1..=100)
        .map(|i| mk_issue(i, "done", Status::Done, Some(7)))
        .collect();
    issues.push(mk_issue(999, "working one", Status::Planned, None));
    let mut m = model_full(
        issues,
        vec![(mk_container(7, "tui plan", None, Some(4)), 0)],
        vec![(mk_container(4, "TUI", Some("0.4.0"), None), 0)],
    );
    m.milestone_directs = vec![(4, 999)];
    m.view = View::MilestoneDetail { milestone_id: 4 };
    let mut terminal = test_backend(100, 30);
    terminal
        .draw(|f| draw_detail(f, &mut m, 4, f.area()))
        .unwrap();
    let text = buffer_text(terminal.backend().buffer()).join("\n");
    assert!(text.contains("working 1%"), "present 组最小 1%: {text}");
}

#[test]
fn milestone_detail_shows_page_numbers_when_paged() {
    let plans: Vec<(Container, i64)> = (1..=12)
        .map(|i| (mk_container(i, &format!("plan {i}"), None, Some(4)), 0))
        .collect();
    let mut m = model_full(
        vec![],
        plans,
        vec![(mk_container(4, "TUI", Some("0.4.0"), None), 0)],
    );
    m.view = View::MilestoneDetail { milestone_id: 4 };
    let mut terminal = test_backend(100, 30);
    terminal
        .draw(|f| draw_detail(f, &mut m, 4, f.area()))
        .unwrap();
    let text = buffer_text(terminal.backend().buffer()).join("\n");
    assert!(text.contains("plans · page 1/2"), "plans 页码: {text}");
    assert!(text.contains("issues · page 1/1"), "issues 页码: {text}");
}

#[test]
fn plan_row_title_truncates_when_long() {
    let mut m = model_full(
        vec![mk_issue(1, "a", Status::Done, Some(7))],
        vec![(
            mk_container(
                7,
                "一个非常非常非常非常非常长的 plan 标题用于验证省略",
                None,
                Some(4),
            ),
            0,
        )],
        vec![(mk_container(4, "TUI", Some("0.4.0"), None), 0)],
    );
    m.view = View::MilestoneDetail { milestone_id: 4 };
    let mut terminal = test_backend(80, 20);
    terminal
        .draw(|f| draw_detail(f, &mut m, 4, f.area()))
        .unwrap();
    let lines = buffer_text(terminal.backend().buffer());
    let plan_row = lines.iter().find(|l| l.contains("#7")).expect("plan 行");
    assert!(plan_row.contains('…'), "长 plan 标题应右侧省略: {plan_row}");
}

#[test]
fn milestone_body_truncates_to_10_lines() {
    let mut m = model_full(
        vec![mk_issue(1, "open one", Status::Open, Some(7))],
        vec![(mk_container(7, "tui plan", None, Some(4)), 0)],
        vec![(mk_container(4, "TUI", Some("0.4.0"), None), 0)],
    );
    m.milestones[0].0.body = Some(
        (1..=20)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    m.view = View::MilestoneDetail { milestone_id: 4 };
    let mut terminal = test_backend(100, 40);
    terminal
        .draw(|f| draw_detail(f, &mut m, 4, f.area()))
        .unwrap();
    let text = buffer_text(terminal.backend().buffer()).join("\n");
    assert!(text.contains("line 1"), "body 开头: {text}");
    assert!(text.contains("line 10…"), "末行省略: {text}");
    assert!(!text.contains("line 11"), "超限行省略: {text}");
}

#[test]
fn milestone_detail_omits_issues_panel_without_any_issue() {
    let mut m = model_full(
        vec![],
        vec![(mk_container(7, "tui plan", None, Some(4)), 0)],
        vec![(mk_container(4, "TUI", Some("0.4.0"), None), 0)],
    );
    let mut terminal = test_backend(100, 20);
    terminal
        .draw(|f| draw_detail(f, &mut m, 4, f.area()))
        .unwrap();
    let text = buffer_text(terminal.backend().buffer()).join("\n");
    assert!(
        text.contains("no issues in this milestone"),
        "无任何 issue 时 issues panel 显示空提示: {text}"
    );
}

/// #240：plan 行进度条复用 progress_bar——含 dropped issue 时出现红色 dropped 段。
#[test]
fn plan_row_progress_bar_shows_dropped_red_segment() {
    // plan 7 含 1 done + 1 dropped → 行内进度条应有红色 dropped 段（非无色 mini_bar）。
    let mut m = model_full(
        vec![
            mk_issue(1, "done in plan", Status::Done, Some(7)),
            mk_issue(2, "dropped in plan", Status::Dropped, Some(7)),
        ],
        vec![(mk_container(7, "tui plan", None, Some(4)), 0)],
        vec![(mk_container(4, "TUI", Some("0.4.0"), None), 0)],
    );
    m.view = View::MilestoneDetail { milestone_id: 4 };
    let mut terminal = test_backend(80, 20);
    terminal
        .draw(|f| draw_detail(f, &mut m, 4, f.area()))
        .unwrap();
    let buf = terminal.backend().buffer();
    let lines = buffer_text(buf);
    let y = lines
        .iter()
        .position(|l| l.contains("#7"))
        .expect("plan 行");
    let has_red = (0..buf.area.width).any(|x| buf[(x, y as u16)].fg == Color::Red);
    assert!(
        has_red,
        "plan 行进度条应含 dropped 红色段（复用 progress_bar）"
    );
}
