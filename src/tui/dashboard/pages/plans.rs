//! plans 页面：milestone 分组 panel（执行中 milestone → 无 milestone → 剩余 milestone）。

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::pages::common::{flash_style, mini_bar};
use crate::tui::dashboard::types::JumpKind;
use crate::tui::panel::{render_panel, stack};

/// Plans tab：按 milestone 分组渲染（每组一 panel 标题），行选中按展平行号映射。
pub fn draw_plans_panel(frame: &mut Frame, m: &DashboardModel, area: Rect) {
    let groups = m.plan_groups();
    let start = m.page * m.page_size;
    let end = start + m.page_size;

    let mut lines: Vec<Line> = Vec::new();
    let mut n = 0; // 展平行号（全局，仅计 plan 行）
    for g in &groups {
        let g_start = n;
        let g_end = n + g.plans.len();
        n = g_end;
        // 组标题仅在页内有该组行时显示。
        if g_end <= start || g_start >= end {
            continue;
        }
        lines.push(Line::from(Span::styled(
            g.title.as_str(),
            Style::new().add_modifier(Modifier::BOLD),
        )));
        for (i, (plan, _)) in g.plans.iter().enumerate() {
            let global = g_start + i;
            if global < start || global >= end {
                continue;
            }
            let selected = global - start == m.selected;
            let (done, total) = m.plan_progress(plan.id);
            let mut style = if selected {
                Style::new().add_modifier(Modifier::REVERSED)
            } else {
                Style::new()
            };
            if let Some(fs) = flash_style(m, plan.id, JumpKind::Plan) {
                style = style.patch(fs);
            }
            let bar = mini_bar(done, total, 20);
            lines.push(Line::from(vec![
                Span::styled(format!("#{:<3}", plan.id), style),
                Span::styled(bar, style),
                Span::styled(format!(" {done}/{total}  {}", plan.title), style),
            ]));
        }
    }
    if lines.is_empty() {
        lines.push(Line::from("(no plans)"));
    }

    let footer = format!(
        "j/k ↑↓ plan · ←/→ page · 1/2/3 tab · Enter detail · q quit · Page {}/{}",
        m.page + 1,
        m.pages()
    );
    let chunks = stack(area, &[Constraint::Min(0), Constraint::Length(1)]);
    let title = format!("plans · page {}/{}", m.page + 1, m.pages());
    render_panel(frame, chunks[0], &title, lines);
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
    fn draw_plans_panel_groups_by_milestone() {
        let mut m = model_full(
            vec![mk_issue(1, "a", Status::Done, Some(7))],
            vec![(mk_container(7, "tui plan", None, Some(4)), 0)],
            vec![(mk_container(4, "TUI", Some("0.4.0"), None), 0)],
        );
        m.view = View::Plans;
        let mut terminal = test_backend(70, 12);
        terminal
            .draw(|f| draw_plans_panel(f, &m, f.area()))
            .unwrap();
        let text = buffer_text(terminal.backend().buffer()).join("\n");
        assert!(text.contains("plans"), "标题: {text}");
        assert!(text.contains("TUI (0.4.0)"), "组标题: {text}");
        assert!(text.contains("tui plan"), "plan 标题: {text}");
        assert!(text.contains("1/1"), "进度: {text}");
    }

    #[test]
    fn draw_plans_panel_lists_no_milestone_group() {
        let mut m = model_full(
            vec![mk_issue(1, "a", Status::Done, Some(7))],
            vec![(mk_container(7, "free plan", None, None), 0)],
            vec![],
        );
        m.view = View::Plans;
        let mut terminal = test_backend(70, 12);
        terminal
            .draw(|f| draw_plans_panel(f, &m, f.area()))
            .unwrap();
        let text = buffer_text(terminal.backend().buffer()).join("\n");
        assert!(text.contains("no milestone"), "组标题: {text}");
        assert!(text.contains("free plan"), "plan 标题: {text}");
    }
}
