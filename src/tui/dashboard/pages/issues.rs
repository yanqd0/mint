//! issues 页面：进度条（open 率）+ 状态点列表（Issue/Plan 面板共用）。

use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Rect};
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
    // 列宽按内容预估值（ID Min 完整展示、VERSION Max 弹性、其余 Length 固定），框架处理间隙。
    let widths = [
        Constraint::Length(1), // 状态点 ●
        Constraint::Min(4),    // ID（#123，关键信息完整展示，弹性增长）
        Constraint::Length(6), // STATUS（plan/drop 4 字符）
        Constraint::Length(1), // P（0-3 单字符）
        Constraint::Length(5), // KIND（req/bug/task）
        Constraint::Max(9),    // VERSION（所属 milestone version，如 v0.111.1，弹性上限 9）
        Constraint::Length(14),
        Constraint::Fill(1), // TITLE（缓冲，Fill 优先级最低，最后拿剩余）
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
            // VERSION：直属 milestone（issue.direct_milestone）版本优先；无直属则回退所属 plan 的
            // milestone version（plan.milestone_id → milestone.version）；两者皆无则空。
            let version = i
                .direct_milestone
                .or_else(|| {
                    i.plan_id
                        .and_then(|pid| m.plans.iter().find(|(p, _)| p.id == pid))
                        .and_then(|(p, _)| p.milestone_id)
                })
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
    let table = Table::new(rows, widths)
        .header(header)
        .flex(Flex::Legacy) // Min/Max 列按内容预估，Fill(TITLE) 拿剩余（默认 Start 会拉伸 Min 列）
        .block(
            Block::bordered()
                .title(panel_list_title)
                .padding(Padding::horizontal(1)),
        );
    frame.render_widget(table, chunks[1]);
    frame.render_widget(Paragraph::new(footer), chunks[2]);
}

#[cfg(test)]
#[path = "issues_tests.rs"]
mod tests;
