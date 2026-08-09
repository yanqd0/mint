//! milestones 页面：milestone 自身标题 + 其下 plan 行列表（每行迷你进度 + done/total）。

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::pages::common::mini_bar;
use crate::tui::dashboard::types::View;

/// 渲染 milestone 面板：自身标题 + plan 行列表（每行迷你进度条 + done/total）。
pub fn draw_milestone(frame: &mut Frame, m: &DashboardModel) {
    let View::Milestone { milestone_id } = m.view else {
        return;
    };
    let title = m
        .milestones
        .iter()
        .find(|(c, _)| c.id == milestone_id)
        .map(|(c, _)| match &c.version {
            Some(v) => format!("{} ({v})", c.title),
            None => c.title.clone(),
        })
        .unwrap_or_else(|| format!("#{milestone_id}"));

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
        lines.push(Line::from("(no plans in this milestone)"));
    }

    let footer = format!(
        "j/k ↑↓ plan · h/l PgUp/PgDn page · Enter plan · Esc back · q quit · Page {}/{}",
        m.page + 1,
        m.plan_pages()
    );
    let chunks = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(frame.area());
    frame.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(format!("mint · milestone {title}"))),
        chunks[0],
    );
    frame.render_widget(Paragraph::new(Line::from(footer)), chunks[1]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Status;
    use crate::tui::dashboard::pages::tests_common::{buffer_text, mk_container, mk_issue, model_full, test_backend};

    #[test]
    fn draw_milestone_panel_shows_plan_rows_with_progress() {
        let mut m = model_full(
            vec![
                mk_issue(1, "done one", Status::Done, Some(7)),
                mk_issue(2, "open one", Status::Open, Some(7)),
            ],
            vec![(mk_container(7, "tui plan", None, Some(4)), 0)],
            vec![(mk_container(4, "TUI", Some("0.4.0"), None), 0)],
        );
        m.view = View::Milestone { milestone_id: 4 };
        m.selected = 0;
        let mut terminal = test_backend(70, 10);
        terminal.draw(|f| draw_milestone(f, &m)).unwrap();
        let text = buffer_text(terminal.backend().buffer()).join("\n");
        assert!(text.contains("milestone TUI (0.4.0)"), "标题: {text}");
        assert!(text.contains("tui plan"), "plan 标题: {text}");
        assert!(text.contains("1/2"), "进度: {text}");
    }
}
