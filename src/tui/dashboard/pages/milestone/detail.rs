//! milestone 详情页：自身信息 + 其下 plan 列表 + issue 聚合（三 panel）。

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::models::Status;
use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::pages::common::{container_status_color, mini_bar};
use crate::tui::dashboard::pages::issues;
use crate::tui::panel::{render_panel, stack};

/// MilestoneDetail：自身信息 / plan 列表 / issue 聚合（复用 issues 页）三个 panel。
pub fn draw_detail(frame: &mut Frame, m: &DashboardModel, milestone_id: i64, area: Rect) {
    // 1. 自身信息 panel。
    let issues: Vec<&crate::models::Issue> = m.visible_issues();
    let total = issues
        .iter()
        .filter(|i| !matches!(i.status, Status::Dropped))
        .count();
    let done = issues.iter().filter(|i| i.status == Status::Done).count();
    let mut info_lines = Vec::new();
    if let Some((c, _)) = m.milestones.iter().find(|(c, _)| c.id == milestone_id) {
        let ver = c.version.as_deref().unwrap_or("");
        info_lines.push(Line::from(vec![
            Span::raw(format!("#{} ", c.id)),
            Span::styled(c.title.as_str(), Style::new().add_modifier(Modifier::BOLD)),
            Span::styled(format!("  ({ver})"), Style::new()),
        ]));
        info_lines.push(Line::from(vec![
            Span::raw("  status: "),
            Span::styled(
                c.status.as_str(),
                Style::new().fg(container_status_color(c.status)),
            ),
            Span::raw(format!("   progress: {done}/{total}")),
        ]));
    }

    // 2. 其下 plan 列表 panel（每行迷你进度 + done/total）。
    let mut plan_lines: Vec<Line> = Vec::new();
    for (plan, _) in m.milestone_plans(milestone_id) {
        let (pdone, ptotal) = m.plan_progress(plan.id);
        let bar = mini_bar(pdone, ptotal, 20);
        plan_lines.push(Line::from(vec![
            Span::raw(format!("#{:<3}", plan.id)),
            Span::raw(format!("{bar}")),
            Span::raw(format!(" {pdone}/{ptotal}  {}", plan.title)),
        ]));
    }
    if plan_lines.is_empty() {
        plan_lines.push(Line::from("(no plans in this milestone)"));
    }

    // 3. issue 聚合 panel（复用 issues 页）。
    let chunks = stack(
        area,
        &[
            Constraint::Length(4),
            Constraint::Min(0),
            Constraint::Length(10),
            Constraint::Length(1),
        ],
    );
    render_panel(
        frame,
        chunks[0],
        &format!("milestone #{milestone_id}"),
        info_lines,
    );
    render_panel(frame, chunks[1], "plans", plan_lines);
    issues::draw_issues_panel(frame, m, chunks[2]);
    frame.render_widget(
        Paragraph::new(Line::from("Esc back · 1/2/3 tab · q quit")),
        chunks[3],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::dashboard::pages::tests_common::{
        buffer_text, mk_container, mk_issue, model_full, test_backend,
    };
    use crate::tui::dashboard::types::View;

    #[test]
    fn milestone_detail_shows_info_plans_and_issues() {
        let mut m = model_full(
            vec![
                mk_issue(1, "open one", Status::Open, Some(7)),
                mk_issue(2, "done one", Status::Done, Some(7)),
            ],
            vec![(mk_container(7, "tui plan", None, Some(4)), 0)],
            vec![(mk_container(4, "TUI", Some("0.4.0"), None), 0)],
        );
        m.view = View::MilestoneDetail { milestone_id: 4 };
        let mut terminal = test_backend(100, 20);
        terminal.draw(|f| draw_detail(f, &m, 4, f.area())).unwrap();
        let text = buffer_text(terminal.backend().buffer()).join("\n");
        assert!(text.contains("milestone #4"), "标题: {text}");
        assert!(text.contains("TUI"), "info: {text}");
        assert!(text.contains("0.4.0"), "version: {text}");
        assert!(text.contains("plans"), "plan 列表标题: {text}");
        assert!(text.contains("tui plan"), "plan 行: {text}");
    }
}
