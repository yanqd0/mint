//! plan 详情页：自身信息 panel + kanban（6 态分列）+ issue list panel。

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::models::{Issue, Status};
use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::pages::common::{container_status_color, truncate};
use crate::tui::dashboard::pages::issues;
use crate::tui::panel::{columns, render_panel, stack};

/// kanban 全列状态（6 态顺序）。
const STATUSES: [Status; 6] = [
    Status::Open,
    Status::Planned,
    Status::Dev,
    Status::Test,
    Status::Done,
    Status::Dropped,
];

/// PlanDetail：自身信息 / kanban / issue list 三个 panel。
pub fn draw_detail(frame: &mut Frame, m: &DashboardModel, plan_id: i64, area: Rect) {
    // 1. 自身信息 panel。
    let (done, total) = m.plan_progress(plan_id);
    let mut info_lines = Vec::new();
    if let Some((c, _)) = m.plans.iter().find(|(c, _)| c.id == plan_id) {
        info_lines.push(Line::from(vec![
            Span::raw(format!("#{} ", c.id)),
            Span::styled(c.title.as_str(), Style::new().add_modifier(Modifier::BOLD)),
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

    // 2. kanban panel（6 态分列，ID+截断标题）。
    let plan_issues: Vec<&Issue> = m.visible_issues();
    let kanban_cols: Vec<(String, Vec<String>)> = STATUSES
        .iter()
        .map(|s| {
            let items: Vec<&Issue> = plan_issues
                .iter()
                .copied()
                .filter(|i| i.status == *s)
                .collect();
            let rows: Vec<String> = items
                .iter()
                .map(|i| format!("#{} {}", i.id, truncate(&i.title, 8)))
                .collect();
            (format!("{} ({})", s.as_str(), rows.len()), rows)
        })
        .collect();

    let chunks = stack(
        area,
        &[
            Constraint::Length(4),
            Constraint::Min(0),
            Constraint::Length(10),
            Constraint::Length(1),
        ],
    );
    render_panel(frame, chunks[0], &format!("plan #{plan_id}"), info_lines);

    let k_cols = columns(chunks[1], &[Constraint::Percentage(17); 6]);
    for (i, (title, rows)) in kanban_cols.iter().enumerate() {
        let mut lines: Vec<Line> = rows.iter().map(|r| Line::from(r.clone())).collect();
        if lines.is_empty() {
            lines.push(Line::from("(empty)"));
        }
        render_panel(frame, k_cols[i], title, lines);
    }

    // 3. issue list panel（复用 issues 页）。
    issues::draw_issues_panel(frame, m, chunks[2]);
    // 4. footer。
    frame.render_widget(
        Paragraph::new(Line::from("Esc back · 1/2/3 tab · q quit")),
        chunks[3],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::dashboard::pages::common::truncate;
    use crate::tui::dashboard::pages::tests_common::{
        buffer_text, mk_container, mk_issue, model_full, test_backend,
    };

    #[test]
    fn truncate_keeps_width_budget() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("hello world", 5), "hell…");
    }

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
        let mut terminal = test_backend(120, 20);
        terminal.draw(|f| draw_detail(f, &m, 7, f.area())).unwrap();
        let text = buffer_text(terminal.backend().buffer()).join("\n");
        assert!(text.contains("plan #7"), "标题: {text}");
        assert!(text.contains("tui plan"), "info: {text}");
        assert!(text.contains("open (1)"), "kanban open 列: {text}");
        assert!(text.contains("done (1)"), "kanban done 列: {text}");
        assert!(text.contains("#1 open"), "kanban 行: {text}");
    }
}
