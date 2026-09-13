//! plan_detail.rs 拆分的独立测试模块。

use super::*;
use crate::tui::dashboard::pages::tests_common::{
    buffer_text, mk_container, mk_issue, model_full, test_backend,
};

#[test]
fn plan_detail_shows_info_and_kanban_columns() {
    let mut m = model_full(
        vec![
            mk_issue(1, "open task", Status::Open, Some(7)),
            mk_issue(2, "done task", Status::Done, Some(7)),
        ],
        vec![(mk_container(7, "tui plan", None, None), 0)],
        vec![],
    );
    m.view = crate::tui::dashboard::types::View::PlanDetail { plan_id: 7 };
    let mut terminal = test_backend(120, 24);
    terminal
        .draw(|f| draw_detail(f, &mut m, 7, f.area()))
        .unwrap();
    let text = buffer_text(terminal.backend().buffer()).join("\n");
    assert!(
        text.contains("issues · page"),
        "列表标题应为 issues: {text}"
    );
    assert!(
        !text.contains("plan #7 · page"),
        "列表标题不应是 plan #X: {text}"
    );
    assert!(text.contains("tui plan"), "info: {text}");
    assert!(text.contains("open (1)"), "kanban open 列: {text}");
    assert!(text.contains("done (1)"), "kanban done 列: {text}");
    assert!(text.contains("#1 open"), "kanban 行: {text}");
}

#[test]
fn kanban_column_titles_fit_or_ellipsis_at_narrow_width() {
    // 5 态各 1 issue（无 dropped 列），窄宽下列宽不足以容纳全部列标题。
    let mut m = model_full(
        vec![
            mk_issue(1, "a", Status::Open, Some(7)),
            mk_issue(2, "b", Status::Planned, Some(7)),
            mk_issue(3, "c", Status::Dev, Some(7)),
            mk_issue(4, "d", Status::Test, Some(7)),
            mk_issue(5, "e", Status::Done, Some(7)),
        ],
        vec![(mk_container(7, "tui plan", None, None), 0)],
        vec![],
    );
    m.view = crate::tui::dashboard::types::View::PlanDetail { plan_id: 7 };
    let mut terminal = test_backend(60, 20);
    terminal
        .draw(|f| draw_detail(f, &mut m, 7, f.area()))
        .unwrap();
    let lines = buffer_text(terminal.backend().buffer());
    // 按内容定位 kanban 头行（不硬编码行号——布局变动不致断言失效）。
    let header = lines
        .iter()
        .find(|l| l.contains("open (1)"))
        .expect("kanban 头行应含 open 列标题");
    // 标题完整或右侧省略，不硬切缺字符/右角（如 "open (1" 缺 ")"）。
    assert!(header.contains("open (1)"), "open 标题应完整: {header}");
    assert!(header.contains('…'), "窄宽下列标题应右侧省略: {header}");
}

/// #342：kanban 行数对齐面板内容区（面板高 10 - 2 border = 8 行），
/// 溢出用 `…` 占用末行，不越界裁剪（此前取 10 行 + 独立 `…` 共 11 行被裁）。
#[test]
fn kanban_rows_fit_panel_height_with_ellipsis() {
    // 12 个 open issue：内容区 8 行 → 显示 7 行 + `…`。
    let issues: Vec<Issue> = (1..=12)
        .map(|id| mk_issue(id, &format!("open-{id}"), Status::Open, Some(7)))
        .collect();
    let mut m = model_full(
        issues,
        vec![(mk_container(7, "tui plan", None, None), 0)],
        vec![],
    );
    m.view = crate::tui::dashboard::types::View::PlanDetail { plan_id: 7 };
    let mut terminal = test_backend(120, 24);
    terminal
        .draw(|f| draw_detail(f, &mut m, 7, f.area()))
        .unwrap();
    let text = buffer_text(terminal.backend().buffer()).join("\n");
    assert!(text.contains("open (12)"), "kanban 头显示总数: {text}");
    assert!(text.contains('…'), "溢出应有 …: {text}");
    // 末 issue 不被显示（超出内容区 8 行），且第 8 行数据应显示（7 行数据 + …）。
    assert!(!text.contains("#12 open"), "超出内容区不应显示: {text}");
}

/// #348：dev+test 都空 → 合并一列 `dev | test (0)`；无独立 dev/test 列。
#[test]
fn kanban_merges_empty_dev_test() {
    let mut m = model_full(
        vec![mk_issue(1, "t", Status::Done, Some(7))],
        vec![(mk_container(7, "tui plan", None, None), 0)],
        vec![],
    );
    m.view = crate::tui::dashboard::types::View::PlanDetail { plan_id: 7 };
    let mut terminal = test_backend(120, 24);
    terminal
        .draw(|f| draw_detail(f, &mut m, 7, f.area()))
        .unwrap();
    let text = buffer_text(terminal.backend().buffer()).join("\n");
    assert!(text.contains("dev | test (0)"), "合并列标题: {text}");
    // 逐行断言无独立列（合并标题含子串 "test (0)"，不能直接用 contains 反向断言）。
    assert!(
        text.lines().all(|l| !l.trim_start().starts_with("dev (0)")),
        "无独立 dev 列: {text}"
    );
    assert!(
        text.lines()
            .all(|l| !l.trim_start().starts_with("test (0)")),
        "无独立 test 列: {text}"
    );
}

/// #348：dev/test 非空 → 各自独立列（不合并）。
#[test]
fn kanban_dev_test_not_merged_when_nonempty() {
    let mut m = model_full(
        vec![mk_issue(1, "d", Status::Dev, Some(7))],
        vec![(mk_container(7, "tui plan", None, None), 0)],
        vec![],
    );
    m.view = crate::tui::dashboard::types::View::PlanDetail { plan_id: 7 };
    let mut terminal = test_backend(120, 24);
    terminal
        .draw(|f| draw_detail(f, &mut m, 7, f.area()))
        .unwrap();
    let text = buffer_text(terminal.backend().buffer()).join("\n");
    assert!(text.contains("dev (1)"), "dev 独立列: {text}");
    assert!(text.contains("test (0)"), "test 独立列: {text}");
    assert!(!text.contains("dev | test"), "不应合并: {text}");
}

/// #348：open+planned 都空且 dropped 有内容 → 合并 `open | planned (0)`。
#[test]
fn kanban_merges_empty_open_planned_when_dropped_present() {
    let mut m = model_full(
        vec![mk_issue(1, "x", Status::Dropped, Some(7))],
        vec![(mk_container(7, "tui plan", None, None), 0)],
        vec![],
    );
    m.view = crate::tui::dashboard::types::View::PlanDetail { plan_id: 7 };
    let mut terminal = test_backend(120, 24);
    terminal
        .draw(|f| draw_detail(f, &mut m, 7, f.area()))
        .unwrap();
    let text = buffer_text(terminal.backend().buffer()).join("\n");
    assert!(text.contains("open | planned (0)"), "合并列标题: {text}");
    assert!(
        text.lines()
            .all(|l| !l.trim_start().starts_with("open (0)")),
        "无独立 open 列: {text}"
    );
    assert!(
        text.lines()
            .all(|l| !l.trim_start().starts_with("planned (0)")),
        "无独立 planned 列: {text}"
    );
    // dropped 有内容 → 显示 dropped 列。
    assert!(text.contains("dropped (1)"), "dropped 列显示: {text}");
}

/// #348：dropped 空 → dropped 列隐藏（既有规则）；open+planned 空但 dropped 空 → 独立空列。
#[test]
fn kanban_dropped_empty_hidden() {
    let mut m = model_full(
        vec![mk_issue(1, "t", Status::Done, Some(7))],
        vec![(mk_container(7, "tui plan", None, None), 0)],
        vec![],
    );
    m.view = crate::tui::dashboard::types::View::PlanDetail { plan_id: 7 };
    let mut terminal = test_backend(120, 24);
    terminal
        .draw(|f| draw_detail(f, &mut m, 7, f.area()))
        .unwrap();
    let text = buffer_text(terminal.backend().buffer()).join("\n");
    assert!(!text.contains("dropped (0)"), "dropped 空列隐藏: {text}");
    // dropped 空 → open+planned 独立空列（不满足规则2 的 dropped 有内容条件）。
    assert!(text.contains("open (0)"), "open 独立列: {text}");
    assert!(text.contains("planned (0)"), "planned 独立列: {text}");
}

#[test]
fn plan_detail_shows_body_panel_when_present() {
    let mut m = model_full(
        vec![mk_issue(1, "task", Status::Open, Some(7))],
        vec![(mk_container(7, "tui plan", None, None), 0)],
        vec![],
    );
    m.plans[0].0.body = Some("plan body content".into());
    m.view = crate::tui::dashboard::types::View::PlanDetail { plan_id: 7 };
    let mut terminal = test_backend(120, 24);
    terminal
        .draw(|f| draw_detail(f, &mut m, 7, f.area()))
        .unwrap();
    let text = buffer_text(terminal.backend().buffer()).join("\n");
    assert!(text.contains("plan body content"), "body panel: {text}");
}

#[test]
fn plan_detail_body_truncates_to_10_lines() {
    let mut m = model_full(
        vec![mk_issue(1, "task", Status::Open, Some(7))],
        vec![(mk_container(7, "tui plan", None, None), 0)],
        vec![],
    );
    m.plans[0].0.body = Some(
        (1..=20)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    m.view = crate::tui::dashboard::types::View::PlanDetail { plan_id: 7 };
    let mut terminal = test_backend(120, 30);
    terminal
        .draw(|f| draw_detail(f, &mut m, 7, f.area()))
        .unwrap();
    let text = buffer_text(terminal.backend().buffer()).join("\n");
    assert!(text.contains("line 1"), "body 开头: {text}");
    assert!(text.contains("line 10…"), "末行省略: {text}");
    assert!(!text.contains("line 11"), "超限行省略: {text}");
}
