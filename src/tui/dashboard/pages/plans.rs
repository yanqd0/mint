//! plans 页面：Plans tab，plan 行列表（每行迷你进度 + done/total）。

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::pages::common::mini_bar;

/// Plans tab：全部 plan 行（每行迷你进度条 + done/total + 标题）。
pub fn draw_plans_panel(frame: &mut Frame, m: &DashboardModel, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();
    for (idx, (plan, _)) in m.page_plans().iter().enumerate() {
        let (done, total) = m.plan_progress(plan.id);
        let selected = idx == m.selected;
        let style = if selected {
            Style::new().add_modifier(Modifier::REVERSED)
        } else {
            Style::new()
        };
        let bar = mini_bar(done, total, 20);
        lines.push(Line::from(vec![
            Span::styled(format!("#{:<3}", plan.id), style),
            Span::styled(format!("[{bar}]"), style),
            Span::styled(format!(" {done}/{total}  {}", plan.title), style),
        ]));
    }
    if lines.is_empty() {
        lines.push(Line::from("(no plans)"));
    }
    let footer = format!(
        "j/k ↑↓ plan · h/l page · 1/2/3 tab · Enter detail · q quit · Page {}/{}",
        m.page + 1,
        m.pages()
    );
    let chunks = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(area);
    frame.render_widget(
        Paragraph::new(lines).block(Block::bordered().title("mint · plans")),
        chunks[0],
    );
    frame.render_widget(Paragraph::new(Line::from(footer)), chunks[1]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Status;
    use crate::tui::dashboard::pages::tests_common::{
        buffer_text, mk_container, mk_issue, model_full, test_backend,
    };
    use crate::tui::dashboard::types::View;

    #[test]
    fn draw_plans_panel_lists_plans_with_progress() {
        let mut m = model_full(
            vec![mk_issue(1, "a", Status::Done, Some(7))],
            vec![(mk_container(7, "tui plan", None, None), 0)],
            vec![],
        );
        m.view = View::Plans;
        let mut terminal = test_backend(70, 10);
        terminal
            .draw(|f| draw_plans_panel(f, &m, f.area()))
            .unwrap();
        let text = buffer_text(terminal.backend().buffer()).join("\n");
        assert!(text.contains("mint · plans"), "标题: {text}");
        assert!(text.contains("tui plan"), "plan 标题: {text}");
        assert!(text.contains("1/1"), "进度: {text}");
    }
}
