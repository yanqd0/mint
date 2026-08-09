//! issues 页面：进度条（open 率）+ 状态点列表（Issue/Plan 面板共用）。

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::models::Status;
use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::pages::common::{
    flash_style, progress_bar, status_dot, status_text_style, truncate,
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
    let total = all
        .iter()
        .filter(|i| !matches!(i.status, Status::Dropped))
        .count();
    let done = all
        .iter()
        .filter(|i| matches!(i.status, Status::Done))
        .count();
    let progress_rate = done
        .checked_mul(100)
        .and_then(|d| d.checked_div(total))
        .unwrap_or(0);

    // 进度 panel：open 率进度条 + 百分比。
    let mut prog_lines = vec![progress_bar(&all)];
    prog_lines.push(Line::from(format!("  progress: {progress_rate}%")));

    // 列表 panel：表头 + 状态点/状态文本（着色）+ ID + 标签 + 标题。
    let mut list_lines: Vec<Line> = Vec::new();
    list_lines.push(Line::from(" #  STATUS  LABEL      TITLE"));
    for (idx, i) in page.iter().enumerate() {
        let (dot, dot_style) = status_dot(i.status);
        let selected = idx == m.selected;
        let mut row_style = if selected {
            Style::new().add_modifier(Modifier::REVERSED)
        } else {
            Style::new()
        };
        if let Some(fs) = flash_style(m, i.id, JumpKind::Issue) {
            row_style = row_style.patch(fs);
        }
        let label = i
            .labels
            .first()
            .map(|l| truncate(l, 10))
            .unwrap_or_default();
        list_lines.push(
            Line::from(vec![
                Span::styled(format!("{dot} "), dot_style),
                Span::styled(
                    format!("{:<6}", i.status.as_str()),
                    status_text_style(i.status),
                ),
                Span::styled(format!("#{:<3}", i.id), Style::new()),
                Span::styled(format!(" {label:<10}"), Style::new()),
                Span::styled(format!(" {}", truncate(&i.title, 24)), Style::new()),
            ])
            .patch_style(row_style),
        );
    }

    let footer = format!(
        "j/k row · ←/→ page · 1/2/3 tab · Enter detail · p plan · m milestone · q quit · Page {}/{} ({total} issues)",
        m.page + 1,
        m.pages()
    );
    let chunks = stack(
        area,
        &[
            Constraint::Length(4),
            Constraint::Min(0),
            Constraint::Length(1),
        ],
    );
    render_panel(frame, chunks[0], "progress", prog_lines);
    render_panel(frame, chunks[1], &panel_title(m), list_lines);
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
