//! plan 详情页：basic（键值对）+ body + kanban（6 态分列）+ issue list panel。

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::Color;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::models::{Issue, Status};
use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::pages::common::{
    body_lines_capped, container_status_color, kv_lines, panel_wrap,
};
use crate::tui::dashboard::pages::issues;
use crate::tui::panel::{columns, render_panel, render_panel_tight, stack};
use crate::tui::text::truncate;

/// kanban 面板总高（含 border）；内容区行数 = 面板高 - 2（#342 与 take 行数对齐）。
const KANBAN_PANEL_H: u16 = 10;

/// PlanDetail：basic / body / kanban / issue list 四个 panel。
pub fn draw_detail(frame: &mut Frame, m: &mut DashboardModel, plan_id: i64, area: Rect) {
    let Some((c, _)) = m.plans.iter().find(|(c, _)| c.id == plan_id) else {
        render_panel(
            frame,
            area,
            "plan",
            vec![Line::from(format!("#{plan_id} (deleted)"))],
        );
        return;
    };
    let (done, total) = m.plan_progress(plan_id);

    // 1. basic 键值对（有值才显；milestone 显 #N）。
    let mut kv: Vec<(String, Span<'static>)> = vec![
        (
            "status".into(),
            Span::styled(c.status.as_str(), container_status_color(c.status)),
        ),
        ("progress".into(), Span::raw(format!("{done}/{total}"))),
    ];
    if let Some(mid) = c.milestone_id {
        kv.push(("milestone".into(), Span::raw(format!("#{mid}"))));
    }
    if let Some(v) = &c.version {
        kv.push(("version".into(), Span::raw(v.clone())));
    }
    kv.push((
        "created".into(),
        Span::styled(c.created_at.clone(), Color::Magenta),
    ));
    kv.push((
        "updated".into(),
        Span::styled(c.updated_at.clone(), Color::Magenta),
    ));
    let basic_rows = kv_lines(&kv, area.width.saturating_sub(4));

    // body ≤10 行（多余省略，遇 \n 换行 + 贪心 word-wrap）。
    let body_lines: Vec<Line> = c
        .body
        .as_ref()
        .map(|b| {
            body_lines_capped(b, area.width.saturating_sub(4) as usize, 10)
                .into_iter()
                .map(Line::from)
                .collect()
        })
        .unwrap_or_default();

    // 布局：basic + body(≤10) + kanban(内容 8 行) + issues(弹性) + footer。
    // kanban 面板总高 KANBAN_PANEL_H，内容区 = 总高 - 2（border），取行数与之对齐（#342）。
    let mut constraints: Vec<Constraint> = vec![Constraint::Length(basic_rows.len() as u16 + 2)];
    if !body_lines.is_empty() {
        constraints.push(Constraint::Length(body_lines.len() as u16 + 2));
    }
    constraints.push(Constraint::Length(KANBAN_PANEL_H));
    constraints.push(Constraint::Min(0));
    constraints.push(Constraint::Length(1));
    let chunks = stack(area, &constraints);
    let mut ci = 0;

    frame.render_widget(
        panel_wrap(
            &format!("#{} {}", c.id, c.title),
            basic_rows,
            chunks[ci].width,
        ),
        chunks[ci],
    );
    ci += 1;

    if !body_lines.is_empty() {
        render_panel(frame, chunks[ci], "body", body_lines);
        ci += 1;
    }

    // kanban panel（状态分列，ID+截断标题；dropped 空列隐藏省宽）。
    // 列合并（#348）：dev+test 都空合并为 `dev | test`；open+planned 都空且
    // dropped 有内容时合并为 `open | planned`。空列也显示（标题占位，dropped 除外）。
    let plan_issues: Vec<&Issue> = m.visible_issues();
    let has = |st: Status| plan_issues.iter().any(|i| i.status == st);
    // (标题前缀, 成员状态列表)：合并规则应用到相邻状态。
    let mut groups: Vec<(String, Vec<Status>)> = Vec::new();
    if !has(Status::Open) && !has(Status::Planned) && has(Status::Dropped) {
        groups.push(("open | planned".into(), vec![Status::Open, Status::Planned]));
    } else {
        groups.push(("open".into(), vec![Status::Open]));
        groups.push(("planned".into(), vec![Status::Planned]));
    }
    if !has(Status::Dev) && !has(Status::Test) {
        groups.push(("dev | test".into(), vec![Status::Dev, Status::Test]));
    } else {
        groups.push(("dev".into(), vec![Status::Dev]));
        groups.push(("test".into(), vec![Status::Test]));
    }
    groups.push(("done".into(), vec![Status::Done]));
    groups.push(("dropped".into(), vec![Status::Dropped]));

    let kanban_cols: Vec<(String, Vec<String>)> = groups
        .iter()
        .filter(|(_, statuses)| {
            // dropped 无 issue 时不显示该列（把宽度省出来）；其它组空列也显示。
            !(statuses.contains(&Status::Dropped) && statuses.iter().all(|s| !has(*s)))
        })
        .map(|(title, statuses)| {
            let items: Vec<&Issue> = plan_issues
                .iter()
                .copied()
                .filter(|i| statuses.contains(&i.status))
                .collect();
            // title 存完整，渲染时按列宽顶格（右侧）省略。
            // 取行数 = kanban 内容区行数（面板高 - 2 border）；溢出 `…` 占用末行（#342）。
            let avail = KANBAN_PANEL_H.saturating_sub(2) as usize;
            let mut rows: Vec<String> = items
                .iter()
                .take(avail.saturating_sub(1)) // 预留 `…` 行
                .map(|i| format!("#{} {}", i.id, i.title))
                .collect();
            if items.len() > avail.saturating_sub(1) {
                rows.push("…".into());
            }
            (format!("{title} ({})", items.len()), rows)
        })
        .collect();

    let n = kanban_cols.len().max(1);
    let k_cols = columns(chunks[ci], &vec![Constraint::Percentage(100 / n as u16); n]);
    for (i, (title, rows)) in kanban_cols.iter().enumerate() {
        // title 顶格（右侧）省略：按列内容宽（border 2）动态截断。
        let title_w = k_cols[i].width.saturating_sub(2) as usize;
        let mut lines: Vec<Line> = rows
            .iter()
            .map(|r| Line::from(truncate(r, title_w.max(1))))
            .collect();
        if lines.is_empty() {
            lines.push(Line::from("(empty)"));
        }
        render_panel_tight(frame, k_cols[i], title, lines);
    }
    ci += 1;

    // issue list panel（复用 issues 页）。
    issues::draw_issues_panel(frame, m, chunks[ci]);
    ci += 1;
    frame.render_widget(
        Paragraph::new(Line::from("Esc back · 1/2/3 tab · q quit")),
        chunks[ci],
    );
}

#[cfg(test)]
mod tests {
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
}
