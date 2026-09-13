//! issues.rs 拆分的独立测试模块。

use super::*;
use crate::models::Status;
use crate::tui::dashboard::pages::tests_common::{
    buffer_text, mk_container, mk_issue, model_full, model_with, test_backend,
};
use crate::tui::dashboard::types::View;
use ratatui::style::Color;

#[test]
fn label_chips_all_labels_with_colors() {
    let mut colors = std::collections::HashMap::new();
    colors.insert("dev-clean".to_string(), "#1a7f37".to_string());
    colors.insert("TUI".to_string(), "#a371f7".to_string());
    let labels = vec!["dev-clean".to_string(), "TUI".to_string()];
    let spans = label_chips(&labels, &colors, 20);
    assert_eq!(spans.len(), 3, "2 label + 1 空格");
    assert_eq!(spans[0].content, "dev-clean");
    assert_eq!(spans[1].content, " ");
    assert_eq!(spans[2].content, "TUI");
    // chip 样式：bg=记录色。
    assert_eq!(spans[0].style.bg, Some(Color::Rgb(0x1a, 0x7f, 0x37)));
}

#[test]
fn label_chips_truncates_when_over_budget() {
    let colors = std::collections::HashMap::new();
    // 预算 10：一个长 label 超预算 → 直接空（无 chip 放入），或截断逻辑。
    let long = vec!["a-very-long-label".to_string()];
    let spans = label_chips(&long, &colors, 10);
    assert!(spans.is_empty(), "超预算 label 不应放入: {spans:?}");
    // 两个短 label 挤满预算：第一个放入，第二个超 → 追加 …。
    let colors2 = std::collections::HashMap::new();
    let two = vec!["aa".to_string(), "bbbbbbbbbbbb".to_string()];
    let spans2 = label_chips(&two, &colors2, 5);
    assert_eq!(spans2.len(), 2, "aa + …");
    assert_eq!(spans2[0].content, "aa");
    assert_eq!(spans2[1].content, "…");
}

#[test]
fn label_chips_empty_no_spans() {
    let colors = std::collections::HashMap::new();
    let spans = label_chips(&[], &colors, 20);
    assert!(spans.is_empty());
}

#[test]
fn progress_counts_dropped_even_with_all_filter() {
    let mut m = model_with(vec![
        mk_issue(1, "open", Status::Open, None),
        mk_issue(2, "done", Status::Done, None),
        mk_issue(3, "dropped_issue", Status::Dropped, None),
    ]);
    // 模拟 list --tui 默认筛选：all=false 隐藏 done/dropped 行。
    m.filter = Some(crate::tui::dashboard::types::IssueFilter {
        all: false,
        status: None,
        label: None,
        priority: None,
    });
    let mut terminal = test_backend(60, 10);
    terminal
        .draw(|f| draw_issues_panel(f, &mut m, f.area()))
        .unwrap();
    let text = buffer_text(terminal.backend().buffer()).join("\n");
    // 进度用 scope_issues（dropped 计入完成 → done 33% + dropped 33%），不受 all=false 影响。
    assert!(text.contains("dropped 33%"), "进度应含 dropped: {text}");
    // 列表行仍受筛选：dropped 行隐藏。
    assert!(
        !text.contains("dropped_issue"),
        "列表应隐藏 dropped 行: {text}"
    );
}

#[test]
fn draw_issue_panel_shows_title_rate_and_dot() {
    let mut m = model_with(vec![
        mk_issue(1, "open one", Status::Open, None),
        mk_issue(2, "done one", Status::Done, None),
    ]);
    let mut terminal = test_backend(90, 10);
    terminal
        .draw(|f| draw_issues_panel(f, &mut m, f.area()))
        .unwrap();
    let joined = buffer_text(terminal.backend().buffer()).join("\n");
    assert!(joined.contains("issues"), "标题: {joined}");
    assert!(joined.contains("done 50%"), "分组行: {joined}");
    assert!(joined.contains("open one"), "issue 行: {joined}");
    assert!(joined.contains("STATUS"), "表头: {joined}");
    assert!(joined.contains("●"), "状态点: {joined}");
}

#[test]
fn issue_title_truncates_with_ellipsis_in_tab_and_plan_detail() {
    let mut m = model_with(vec![mk_issue(
        1,
        "一个非常非常非常非常非常长的 issue 标题用于验证截断省略行为",
        Status::Open,
        Some(7),
    )]);
    // Issues tab。
    let mut terminal = test_backend(100, 10);
    terminal
        .draw(|f| draw_issues_panel(f, &mut m, f.area()))
        .unwrap();
    let lines = buffer_text(terminal.backend().buffer());
    let row = lines.iter().find(|l| l.contains("#1")).expect("issue 行");
    assert!(row.contains('…'), "Issues tab 长标题应省略: {row}");
    // PlanDetail（复用同一列表面板）。
    let mut m2 = model_with(vec![mk_issue(
        1,
        "一个非常非常非常非常非常长的 issue 标题用于验证截断省略行为",
        Status::Open,
        Some(7),
    )]);
    m2.view = View::PlanDetail { plan_id: 7 };
    let mut terminal2 = test_backend(100, 10);
    terminal2
        .draw(|f| draw_issues_panel(f, &mut m2, f.area()))
        .unwrap();
    let lines2 = buffer_text(terminal2.backend().buffer());
    let row2 = lines2
        .iter()
        .find(|l| l.contains("#1"))
        .expect("plan issue 行");
    assert!(row2.contains('…'), "PlanDetail 长标题应省略: {row2}");
}

#[test]
fn draw_plan_panel_filters_issues() {
    let mut m = model_with(vec![
        mk_issue(1, "in plan", Status::Dev, Some(7)),
        mk_issue(2, "outside", Status::Open, None),
    ]);
    m.view = View::PlanDetail { plan_id: 7 };
    m.selected = 0;
    let mut terminal = test_backend(90, 10);
    terminal
        .draw(|f| draw_issues_panel(f, &mut m, f.area()))
        .unwrap();
    let text = buffer_text(terminal.backend().buffer()).join("\n");
    assert!(text.contains("issues · page"), "列表标题: {text}");
    assert!(text.contains("in plan"), "应含 plan issue: {text}");
    assert!(!text.contains("outside"), "不应含外部 issue: {text}");
}

/// 搜索激活时 footer 显示 /query█（输入光标占位）。
#[test]
fn footer_shows_query_when_search_active() {
    let mut m = model_with(vec![mk_issue(1, "a", Status::Open, None)]);
    m.search = Some(crate::tui::dashboard::types::SearchState {
        active: true,
        text: "foo".into(),
    });
    let mut terminal = test_backend(60, 10);
    terminal
        .draw(|f| draw_issues_panel(f, &mut m, f.area()))
        .unwrap();
    let text = buffer_text(terminal.backend().buffer()).join("\n");
    assert!(text.contains("/foo█"), "搜索激活 footer 显 /foo█: {text}");
}

/// VERSION 列取直属 milestone（direct_milestone）版本；无直属才回退 plan 的 milestone（#445）。
#[test]
fn issue_version_prefers_direct_milestone_then_plan() {
    // plan 1 挂 milestone 7（v0.7.0）；issue 同时直属挂 milestone 9（v2.0.0）。
    let plan = mk_container(1, "plan", None, Some(7));
    let ms7 = mk_container(7, "0.7.0", Some("v0.7.0"), None);
    let ms9 = mk_container(9, "2.0.0", Some("v2.0.0"), None);
    let mut direct = mk_issue(1, "direct ms", Status::Open, Some(1));
    direct.direct_milestone = Some(9);
    let mut in_plan = mk_issue(2, "only plan", Status::Open, Some(1)); // direct_milestone 无
    in_plan.direct_milestone = None;
    let mut m = model_full(
        vec![direct, in_plan],
        vec![(plan, 0)],
        vec![(ms7, 0), (ms9, 0)],
    );
    let mut terminal = test_backend(120, 12);
    terminal
        .draw(|f| draw_issues_panel(f, &mut m, f.area()))
        .unwrap();
    let text = buffer_text(terminal.backend().buffer()).join("\n");
    // 直属优先：显示直属 milestone v2.0.0，而非 plan 的 v0.7.0。
    let row_direct = text.lines().find(|l| l.contains("#1")).expect("direct 行");
    assert!(
        row_direct.contains("v2.0.0"),
        "直属应显 v2.0.0: {row_direct}"
    );
    assert!(
        !row_direct.contains("v0.7.0"),
        "直属不应回退 plan 版本: {row_direct}"
    );
    // 无直属：回退所属 plan 的 milestone v0.7.0。
    let row_plan = text.lines().find(|l| l.contains("#2")).expect("plan 行");
    assert!(
        row_plan.contains("v0.7.0"),
        "仅属 plan 应显 v0.7.0: {row_plan}"
    );
}

/// 无搜索时 footer 含 / search 提示。
#[test]
fn help_footer_mentions_search() {
    let mut m = model_with(vec![mk_issue(1, "a", Status::Open, None)]);
    let mut terminal = test_backend(60, 10);
    terminal
        .draw(|f| draw_issues_panel(f, &mut m, f.area()))
        .unwrap();
    let text = buffer_text(terminal.backend().buffer()).join("\n");
    assert!(text.contains("/ search"), "help footer 含 / search: {text}");
}
