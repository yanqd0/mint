//! issues 页面：进度条（open 率）+ 状态点列表（Issue/Plan 面板共用）。

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Cell, Padding, Paragraph, Row, Table};

use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::model_view;
use crate::tui::dashboard::pages::common::{
    flash_style, flex_col_width, footer_line, kind_abbrev, label_style, list_title, status_abbrev,
    status_dot, status_text_style,
};
use crate::tui::dashboard::pages::progress::{progress_bar, progress_pct_line};
use crate::tui::dashboard::types::JumpKind;
use crate::tui::panel::{render_panel, stack};
use crate::tui::text::{highlight_spans, truncate};

/// 列表面板标题（Issues tab 与 PlanDetail 共用，恒为 issues；页码在调用处拼接）。
fn panel_title() -> String {
    "issues".to_string()
}

/// 生成 label chip Spans（#273）：每个 label 按记录 color 着色，空格分隔。
/// 按 `budget` 显示宽度截断：超出预算时保留已放入的完整 chip + 追加 `…`。
fn label_chips(
    labels: &[String],
    colors: &std::collections::HashMap<String, String>,
    budget: usize,
) -> Vec<Span<'static>> {
    use unicode_width::UnicodeWidthStr;
    if labels.is_empty() {
        return Vec::new();
    }
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut used = 0usize;
    let mut truncated = false;
    for (n, l) in labels.iter().enumerate() {
        let sep = if n == 0 { 0 } else { 1 };
        let lw = l.width();
        // 预留 1 给末尾 `…`（若后续还有 label 或本 chip 超预算）。
        let needs_ellipsis = n + 1 < labels.len() || used + sep + lw > budget;
        if used + sep + lw + (if needs_ellipsis { 1 } else { 0 }) > budget {
            truncated = true;
            break;
        }
        if n > 0 {
            spans.push(Span::raw(" "));
            used += 1;
        }
        let color = colors.get(l).cloned().unwrap_or_default();
        spans.push(Span::styled(l.clone(), label_style(&color)));
        used += lw;
    }
    if truncated && !spans.is_empty() {
        spans.push(Span::raw("…"));
    }
    spans
}

/// 渲染 issues 页面：进度 panel（上）+ 列表 panel（下）+ footer（Issues tab / PlanDetail 共用）。
pub fn draw_issues_panel(frame: &mut Frame, m: &mut DashboardModel, area: Rect) {
    // 布局先定：progress panel + 列表面板高度（列表按可见高度分页）。
    let chunks = stack(
        area,
        &[
            Constraint::Length(4),
            Constraint::Min(0),
            Constraint::Length(1),
        ],
    );
    let rows_avail = chunks[1].height.saturating_sub(3); // 边框 2 + 表头 1
    m.set_page_size(rows_avail as usize);

    // 进度用视图作用域全集（含 done/dropped，不受列表筛选影响）；列表行仍走 page_issues 筛选。
    let all = m.scope_issues();
    let page = m.page_issues();
    let bar_width = chunks[0].width.saturating_sub(4) as usize; // panel 内容宽（border 2 + padding 2）
    let mut prog_lines = vec![progress_bar(&all, bar_width)];
    prog_lines.push(progress_pct_line(&all));

    // 列宽 + TITLE 弹性列实际宽（title 按此预截断、右侧省略，避免长文本溢出/换行）。
    // 定宽列按内容最小需求收缩（VERSION 5 字符/LABEL 短 chip），解放 TITLE。
    let widths = [
        Constraint::Length(2),
        Constraint::Length(5),
        Constraint::Length(6),
        Constraint::Length(2),
        Constraint::Length(5),
        Constraint::Length(6), // VERSION（所属 milestone version，如 0.6.0）
        Constraint::Length(14),
        Constraint::Min(0),
    ];
    let title_w = flex_col_width(area, &widths);

    // 列表 panel：ratatui Table（表头 + 行；列宽按内容，对齐由 Table 按显示宽处理，解决中文/标签歪）。
    let header = Row::new(vec![
        "#", "ID", "STATUS", "P", "KIND", "VERSION", "LABEL", "TITLE",
    ])
    .style(Style::new().add_modifier(Modifier::BOLD));
    let rows: Vec<Row> = page
        .iter()
        .enumerate()
        .map(|(idx, i)| {
            let (dot, dot_style) = status_dot(i.status);
            // LABEL 列：全部 label 按记录 color 着色（#273，chip 效果），按列宽预算截断。
            let label_cell = Cell::from(Line::from(label_chips(&i.labels, &i.label_colors, 14)));
            // 搜索命中高亮（#261）：有搜索词时对 title 命中子串反色。
            let title = truncate(&i.title, title_w.max(1) as usize);
            let title_spans = match model_view::current_search(m) {
                Some(q) => highlight_spans(&title, q, Style::default()),
                None => vec![Span::raw(title)],
            };
            // VERSION：所属 plan 的 milestone version（issue.plan_id → plan.milestone_id → milestone.version；无则空）。
            let version = i
                .plan_id
                .and_then(|pid| m.plans.iter().find(|(p, _)| p.id == pid))
                .and_then(|(p, _)| p.milestone_id)
                .and_then(|mid| m.milestones.iter().find(|(ms, _)| ms.id == mid))
                .and_then(|(ms, _)| ms.version.clone())
                .unwrap_or_default();
            let mut row = Row::new(vec![
                Cell::from(Line::from(vec![
                    Span::styled(format!("{dot}"), dot_style), // 状态点 ●（状态色/闪烁）
                ])),
                Cell::from(format!("#{}", i.id)),
                Cell::from(Line::from(vec![Span::styled(
                    status_abbrev(i.status),
                    status_text_style(i.status),
                )])),
                Cell::from(i.priority.to_string()),
                Cell::from(kind_abbrev(i.kind).to_string()),
                Cell::from(version),
                label_cell,
                Cell::from(Line::from(title_spans)),
            ]);
            if m.selected_idx() == Some(idx) {
                row = row.style(Style::new().add_modifier(Modifier::REVERSED));
            }
            if let Some(fs) = flash_style(m, i.id, JumpKind::Issue) {
                row = row.style(fs);
            }
            row
        })
        .collect();

    let footer = footer_line(
        m,
        "j/k row · ←/→ page · 1/2/3 tab · Enter detail · / search · q quit",
    );
    // 翻页 + size 信息移入列表 panel 标题（#264：统一 list_title helper）。
    let panel_list_title = format!(
        "─{}",
        list_title(
            &panel_title(),
            m.page + 1,
            m.pages(),
            page.len(),
            m.visible_issues().len(),
        )
    );
    render_panel(frame, chunks[0], "progress", prog_lines);
    let table = Table::new(rows, widths).header(header).block(
        Block::bordered()
            .title(panel_list_title)
            .padding(Padding::horizontal(1)),
    );
    frame.render_widget(table, chunks[1]);
    frame.render_widget(Paragraph::new(footer), chunks[2]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Status;
    use crate::tui::dashboard::pages::tests_common::{
        buffer_text, mk_issue, model_with, test_backend,
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
            revert: (0, 0),
        });
        let mut terminal = test_backend(60, 10);
        terminal
            .draw(|f| draw_issues_panel(f, &mut m, f.area()))
            .unwrap();
        let text = buffer_text(terminal.backend().buffer()).join("\n");
        assert!(text.contains("/foo█"), "搜索激活 footer 显 /foo█: {text}");
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
}
