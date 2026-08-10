//! issues 页面：进度条（open 率）+ 状态点列表（Issue/Plan 面板共用）。

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Cell, Padding, Paragraph, Row, Table};

use crate::models::Status;
use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::pages::common::{
    flash_style, kind_abbrev, progress_bar, status_abbrev, status_dot, status_text_style, truncate,
};
use crate::tui::dashboard::types::{JumpKind, View};
use crate::tui::panel::{render_panel, stack};

/// 面板标题（Issues tab 或 PlanDetail）。
fn panel_title(m: &DashboardModel) -> String {
    match m.view {
        View::Issues => "issues".to_string(),
        View::PlanDetail { plan_id } => format!("plan #{plan_id}"),
        _ => "mint".to_string(),
    }
}

/// 渲染 issues 页面：进度 panel（上）+ 列表 panel（下）+ footer（Issues tab / PlanDetail 共用）。
pub fn draw_issues_panel(frame: &mut Frame, m: &DashboardModel, area: Rect) {
    let all = m.visible_issues();
    let page = m.page_issues();
    // dropped 计入完成（需求 10）；total 含 dropped。
    let total = all.len();
    let done = all
        .iter()
        .filter(|i| matches!(i.status, Status::Done | Status::Dropped))
        .count();
    let progress_rate = done
        .checked_mul(100)
        .and_then(|d| d.checked_div(total.max(1)))
        .unwrap_or(0);

    // 布局先定：progress panel 宽度给进度条（按占比分色分段）。
    let chunks = stack(
        area,
        &[
            Constraint::Length(4),
            Constraint::Min(0),
            Constraint::Length(1),
        ],
    );
    let bar_width = chunks[0].width.saturating_sub(4) as usize; // panel 内容宽（border 2 + padding 2）
    let mut prog_lines = vec![progress_bar(&all, bar_width)];
    prog_lines.push(Line::from(format!("progress: {progress_rate}%")));

    // 列表 panel：ratatui Table（表头 + 行；列宽按内容，对齐由 Table 按显示宽处理，解决中文/标签歪）。
    let header = Row::new(vec!["#", "STATUS", "ID", "P", "KIND", "LABEL", "TITLE"])
        .style(Style::new().add_modifier(Modifier::BOLD));
    let rows: Vec<Row> = page
        .iter()
        .enumerate()
        .map(|(idx, i)| {
            let (dot, dot_style) = status_dot(i.status);
            let label = i
                .labels
                .first()
                .map(|l| truncate(l, 20))
                .unwrap_or_default();
            let mut row = Row::new(vec![
                Cell::from(Line::from(vec![
                    Span::styled(format!("{dot}"), dot_style), // 状态点 ●（状态色/闪烁）
                ])),
                Cell::from(Line::from(vec![Span::styled(
                    status_abbrev(i.status),
                    status_text_style(i.status),
                )])),
                Cell::from(format!("#{}", i.id)),
                Cell::from(i.priority.to_string()),
                Cell::from(kind_abbrev(i.kind).to_string()),
                Cell::from(label),
                Cell::from(i.title.clone()),
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
    let widths = [
        Constraint::Length(2),
        Constraint::Length(6),
        Constraint::Length(5),
        Constraint::Length(2),
        Constraint::Length(5),
        Constraint::Length(20),
        Constraint::Min(0),
    ];

    let footer = "j/k row · ←/→ page · 1/2/3 tab · Enter detail · p plan · m milestone · q quit";
    // 翻页信息移入列表 panel 标题（需求：不放 help 栏）。
    let list_title = format!("─{} · page {}/{}", panel_title(m), m.page + 1, m.pages());
    render_panel(frame, chunks[0], "progress", prog_lines);
    let table = Table::new(rows, widths).header(header).block(
        Block::bordered()
            .title(list_title)
            .padding(Padding::horizontal(1)),
    );
    frame.render_widget(table, chunks[1]);
    frame.render_widget(Paragraph::new(Line::from(footer)), chunks[2]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::dashboard::pages::tests_common::{
        buffer_text, mk_issue, model_with, test_backend,
    };

    #[test]
    fn draw_issue_panel_shows_title_rate_and_dot() {
        let m = model_with(vec![
            mk_issue(1, "open one", Status::Open, None),
            mk_issue(2, "done one", Status::Done, None),
        ]);
        let mut terminal = test_backend(60, 10);
        terminal
            .draw(|f| draw_issues_panel(f, &m, f.area()))
            .unwrap();
        let joined = buffer_text(terminal.backend().buffer()).join("\n");
        assert!(joined.contains("issues"), "标题: {joined}");
        assert!(joined.contains("progress: 50%"), "rate: {joined}");
        assert!(joined.contains("open one"), "issue 行: {joined}");
        assert!(joined.contains("STATUS"), "表头: {joined}");
        assert!(joined.contains("●"), "状态点: {joined}");
    }

    #[test]
    fn draw_plan_panel_filters_issues() {
        let mut m = model_with(vec![
            mk_issue(1, "in plan", Status::Dev, Some(7)),
            mk_issue(2, "outside", Status::Open, None),
        ]);
        m.view = View::PlanDetail { plan_id: 7 };
        m.selected = 0;
        let mut terminal = test_backend(60, 10);
        terminal
            .draw(|f| draw_issues_panel(f, &m, f.area()))
            .unwrap();
        let text = buffer_text(terminal.backend().buffer()).join("\n");
        assert!(text.contains("plan #7"), "标题: {text}");
        assert!(text.contains("in plan"), "应含 plan issue: {text}");
        assert!(!text.contains("outside"), "不应含外部 issue: {text}");
    }
}
