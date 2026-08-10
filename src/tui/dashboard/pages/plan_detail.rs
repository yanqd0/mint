//! plan 详情页：basic（键值对）+ body + kanban（6 态分列）+ issue list panel。

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

use crate::models::{Issue, Status};
use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::pages::common::{body_paragraph, kv_lines, panel_wrap, truncate};
use crate::tui::dashboard::pages::issues;
use crate::tui::panel::{columns, render_panel, render_panel_tight, stack};

/// kanban 全列状态（6 态顺序）。
const STATUSES: [Status; 6] = [
    Status::Open,
    Status::Planned,
    Status::Dev,
    Status::Test,
    Status::Done,
    Status::Dropped,
];

/// PlanDetail：basic / body / kanban / issue list 四个 panel。
pub fn draw_detail(frame: &mut Frame, m: &DashboardModel, plan_id: i64, area: Rect) {
    let Some((c, _)) = m.plans.iter().find(|(c, _)| c.id == plan_id) else {
        render_panel(
            frame,
            area,
            "plan",
            vec![Line::from(format!("#{plan_id} (deleted)"))],
        );
        return;
    };
    let (done, total) = m.plan_progress(plan_id);

    // 1. basic 键值对（有值才显；milestone 显 #N）。
    let mut kv: Vec<(String, String)> = vec![
        ("status".into(), c.status.as_str().to_string()),
        ("progress".into(), format!("{done}/{total}")),
    ];
    if let Some(mid) = c.milestone_id {
        kv.push(("milestone".into(), format!("#{mid}")));
    }
    if let Some(v) = &c.version {
        kv.push(("version".into(), v.clone()));
    }
    kv.push(("created".into(), c.created_at.clone()));
    kv.push(("updated".into(), c.updated_at.clone()));
    let basic_rows = kv_lines(&kv, area.width.saturating_sub(4));

    // 布局：basic + body(有) + kanban(10) + issues(弹性) + footer。
    let mut constraints: Vec<Constraint> = vec![Constraint::Length(basic_rows.len() as u16 + 2)];
    if c.body.is_some() {
        constraints.push(Constraint::Length(4));
    }
    constraints.push(Constraint::Length(10));
    constraints.push(Constraint::Min(0));
    constraints.push(Constraint::Length(1));
    let chunks = stack(area, &constraints);
    let mut ci = 0;

    frame.render_widget(
        panel_wrap(&format!("#{} {}", c.id, c.title), basic_rows),
        chunks[ci],
    );
    ci += 1;

    if let Some(b) = &c.body {
        frame.render_widget(body_paragraph(b, "body"), chunks[ci]);
        ci += 1;
    }

    // kanban panel（6 态分列，ID+截断标题；dropped 空列隐藏省宽）。
    let plan_issues: Vec<&Issue> = m.visible_issues();
    let kanban_cols: Vec<(String, Vec<String>)> = STATUSES
        .iter()
        .filter(|s| {
            // dropped 无 issue 时不显示该列（把宽度省出来）。
            **s != Status::Dropped || plan_issues.iter().any(|i| i.status == **s)
        })
        .map(|s| {
            let items: Vec<&Issue> = plan_issues
                .iter()
                .copied()
                .filter(|i| i.status == *s)
                .collect();
            // title 存完整，渲染时按列宽顶格（右侧）省略。
            let mut rows: Vec<String> = items
                .iter()
                .take(10)
                .map(|i| format!("#{} {}", i.id, i.title))
                .collect();
            if items.len() > 10 {
                rows.push("…".into());
            }
            (format!("{} ({})", s.as_str(), items.len()), rows)
        })
        .collect();

    let n = kanban_cols.len().max(1);
    let k_cols = columns(chunks[ci], &vec![Constraint::Percentage(100 / n as u16); n]);
    for (i, (title, rows)) in kanban_cols.iter().enumerate() {
        // title 顶格（右侧）省略：按列内容宽（border 2）动态截断。
        let title_w = k_cols[i].width.saturating_sub(2) as usize;
        let mut lines: Vec<Line> = rows
            .iter()
            .map(|r| Line::from(truncate(r, title_w.max(1))))
            .collect();
        if lines.is_empty() {
            lines.push(Line::from("(empty)"));
        }
        render_panel_tight(frame, k_cols[i], title, lines);
    }
    ci += 1;

    // issue list panel（复用 issues 页）。
    issues::draw_issues_panel(frame, m, chunks[ci]);
    ci += 1;
    frame.render_widget(
        Paragraph::new(Line::from("Esc back · 1/2/3 tab · q quit")),
        chunks[ci],
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

    #[test]
    fn plan_detail_shows_body_panel_when_present() {
        let mut m = model_full(
            vec![mk_issue(1, "task", Status::Open, Some(7))],
            vec![(mk_container(7, "tui plan", None, None), 0)],
            vec![],
        );
        m.plans[0].0.body = Some("plan body content".into());
        m.view = crate::tui::dashboard::types::View::PlanDetail { plan_id: 7 };
        let mut terminal = test_backend(120, 24);
        terminal.draw(|f| draw_detail(f, &m, 7, f.area())).unwrap();
        let text = buffer_text(terminal.backend().buffer()).join("\n");
        assert!(text.contains("plan body content"), "body panel: {text}");
    }
}
