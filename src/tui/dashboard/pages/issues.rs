//! issues 页面：进度条（open 率）+ 状态点列表（Issue/Plan 面板共用）。

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::models::Status;
use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::pages::common::{progress_bar, status_dot, status_text_style};
use crate::tui::dashboard::types::View;

/// 面板标题（Issue/Plan 视图）。
fn panel_title(m: &DashboardModel) -> String {
    match m.view {
        View::Issue => "mint · issues".to_string(),
        View::Plan { plan_id } => format!("mint · plan #{plan_id}"),
        View::Milestone { milestone_id } => format!("mint · milestone #{milestone_id}"),
    }
}

/// 渲染 issue/plan 面板：进度条 + 状态点列表 + footer。
pub fn draw_issues_panel(frame: &mut Frame, m: &DashboardModel) {
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

    let mut lines: Vec<Line> = Vec::new();
    lines.push(progress_bar(&all));
    lines.push(Line::from(format!("  progress: {progress_rate}%")));
    for (idx, i) in page.iter().enumerate() {
        let (dot, dot_style) = status_dot(i.status);
        let selected = idx == m.selected;
        let text_style = if selected {
            status_text_style(i.status).add_modifier(Modifier::REVERSED)
        } else {
            status_text_style(i.status)
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{dot} "), dot_style),
            Span::styled(
                format!("#{} {} {}", i.id, i.status.as_str(), i.title),
                text_style,
            ),
        ]));
    }

    let footer = format!(
        "j/k ↑↓ row · h/l PgUp/PgDn page · Tab plan · Enter detail · q quit · Page {}/{}",
        m.page + 1,
        m.pages()
    );
    let chunks = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(frame.area());
    frame.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(panel_title(m))),
        chunks[0],
    );
    frame.render_widget(Paragraph::new(Line::from(footer)), chunks[1]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::dashboard::pages::tests_common::{buffer_text, mk_issue, model_with, test_backend};

    #[test]
    fn draw_issue_panel_shows_title_rate_and_dot() {
        let m = model_with(vec![
            mk_issue(1, "open one", Status::Open, None),
            mk_issue(2, "done one", Status::Done, None),
        ]);
        let mut terminal = test_backend(60, 10);
        terminal.draw(|f| draw_issues_panel(f, &m)).unwrap();
        let joined = buffer_text(terminal.backend().buffer()).join("\n");
        assert!(joined.contains("mint · issues"), "标题: {joined}");
        assert!(joined.contains("progress: 50%"), "rate: {joined}");
        assert!(joined.contains("#1 open"), "issue 行: {joined}");
        assert!(joined.contains("●"), "状态点: {joined}");
    }

    #[test]
    fn draw_plan_panel_filters_issues() {
        let mut m = model_with(vec![
            mk_issue(1, "in plan", Status::Dev, Some(7)),
            mk_issue(2, "outside", Status::Open, None),
        ]);
        m.view = View::Plan { plan_id: 7 };
        m.selected = 0;
        let mut terminal = test_backend(60, 10);
        terminal.draw(|f| draw_issues_panel(f, &m)).unwrap();
        let text = buffer_text(terminal.backend().buffer()).join("\n");
        assert!(text.contains("plan #7"), "标题: {text}");
        assert!(text.contains("in plan"), "应含 plan issue: {text}");
        assert!(!text.contains("outside"), "不应含外部 issue: {text}");
    }
}
