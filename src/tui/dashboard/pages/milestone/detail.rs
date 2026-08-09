//! milestone 详情页：自身信息 + 其下 plan 列表 + issue 聚合（三 panel）。

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::models::{Issue, Status};
use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::pages::common::{body_paragraph, kv_lines, mini_bar};
use crate::tui::panel::{render_panel, stack};

/// MilestoneDetail：basic / body / plan 列表 / 直属 issue 列表 四个 panel。
pub fn draw_detail(frame: &mut Frame, m: &DashboardModel, milestone_id: i64, area: Rect) {
    let Some((c, _)) = m.milestones.iter().find(|(c, _)| c.id == milestone_id) else {
        render_panel(
            frame,
            area,
            "milestone",
            vec![Line::from(format!("#{milestone_id} (deleted)"))],
        );
        return;
    };
    let all: Vec<&Issue> = m.visible_issues();
    let total = all
        .iter()
        .filter(|i| !matches!(i.status, Status::Dropped))
        .count();
    let done = all.iter().filter(|i| i.status == Status::Done).count();

    // 1. basic 键值对（有值才显）。
    let mut kv: Vec<(String, String)> = vec![
        ("status".into(), c.status.as_str().to_string()),
        ("progress".into(), format!("{done}/{total}")),
    ];
    if let Some(v) = &c.version {
        kv.push(("version".into(), v.clone()));
    }
    kv.push(("created".into(), c.created_at.clone()));
    kv.push(("updated".into(), c.updated_at.clone()));
    let basic_rows = kv_lines(&kv, area.width.saturating_sub(4));

    // 2. 其下 plan 列表 panel（每行迷你进度 + done/total，最多 5 行省略）。
    let plans = m.milestone_plans(milestone_id);
    let mut plan_lines: Vec<Line> = Vec::new();
    for (plan, _) in plans.iter().take(5) {
        let (pdone, ptotal) = m.plan_progress(plan.id);
        let bar = mini_bar(pdone, ptotal, 20);
        plan_lines.push(Line::from(vec![
            Span::raw(format!("#{:<3}", plan.id)),
            Span::raw(bar),
            Span::raw(format!(" {pdone}/{ptotal}  {}", plan.title)),
        ]));
    }
    if plans.len() > 5 {
        plan_lines.push(Line::from("…"));
    }
    if plan_lines.is_empty() {
        plan_lines.push(Line::from("(no plans in this milestone)"));
    }

    // 3. 直属 issue 列表（milestone_direct_issues，有才显示）。
    let direct_ids: Vec<i64> = m
        .milestone_directs
        .iter()
        .filter(|(mid, _)| *mid == milestone_id)
        .map(|(_, iid)| *iid)
        .collect();
    let direct_lines: Vec<Line> = m
        .issues
        .iter()
        .filter(|i| direct_ids.contains(&i.id))
        .map(|i| Line::from(format!("#{} {}", i.id, i.title)))
        .collect();

    // 布局：basic + body(有) + plans + 直属 issue(有) + footer。
    let mut constraints: Vec<Constraint> = vec![Constraint::Length(basic_rows.len() as u16 + 2)];
    if c.body.is_some() {
        constraints.push(Constraint::Length(4));
    }
    constraints.push(Constraint::Length(plan_lines.len() as u16 + 2));
    if !direct_lines.is_empty() {
        constraints.push(Constraint::Length(direct_lines.len() as u16 + 2));
    }
    constraints.push(Constraint::Length(1));
    let chunks = stack(area, &constraints);
    let mut ci = 0;

    render_panel(
        frame,
        chunks[ci],
        &format!("#{} {}", c.id, c.title),
        basic_rows,
    );
    ci += 1;
    if let Some(b) = &c.body {
        frame.render_widget(body_paragraph(b, "body"), chunks[ci]);
        ci += 1;
    }
    render_panel(frame, chunks[ci], "plans", plan_lines);
    ci += 1;
    if !direct_lines.is_empty() {
        render_panel(frame, chunks[ci], "issues", direct_lines);
        ci += 1;
    }
    frame.render_widget(
        Paragraph::new(Line::from("Esc back · 1/2/3 tab · q quit")),
        chunks[ci],
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
    fn milestone_detail_shows_basic_plans_and_direct_issues() {
        let mut m = model_full(
            vec![
                mk_issue(1, "open one", Status::Open, Some(7)),
                mk_issue(2, "done one", Status::Done, Some(7)),
            ],
            vec![(mk_container(7, "tui plan", None, Some(4)), 0)],
            vec![(mk_container(4, "TUI", Some("0.4.0"), None), 0)],
        );
        m.milestone_directs = vec![(4, 1)]; // issue 1 直属 milestone 4
        m.view = View::MilestoneDetail { milestone_id: 4 };
        let mut terminal = test_backend(100, 20);
        terminal.draw(|f| draw_detail(f, &m, 4, f.area())).unwrap();
        let text = buffer_text(terminal.backend().buffer()).join("\n");
        assert!(text.contains("#4 TUI"), "标题: {text}");
        assert!(text.contains("0.4.0"), "version: {text}");
        assert!(text.contains("plans"), "plan 列表标题: {text}");
        assert!(text.contains("tui plan"), "plan 行: {text}");
        assert!(text.contains("issues"), "直属 issue 列表标题: {text}");
        assert!(text.contains("#1 open one"), "直属 issue 行: {text}");
    }

    #[test]
    fn milestone_detail_omits_issues_panel_without_direct_issues() {
        let m = model_full(
            vec![mk_issue(1, "open one", Status::Open, Some(7))],
            vec![(mk_container(7, "tui plan", None, Some(4)), 0)],
            vec![(mk_container(4, "TUI", Some("0.4.0"), None), 0)],
        );
        let mut terminal = test_backend(100, 20);
        terminal.draw(|f| draw_detail(f, &m, 4, f.area())).unwrap();
        let text = buffer_text(terminal.backend().buffer()).join("\n");
        assert!(
            !text.contains("issues"),
            "无直属 issue 不应有 issues panel: {text}"
        );
    }
}
