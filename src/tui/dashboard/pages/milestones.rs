//! milestones 页面：Milestones tab 列表 + MilestoneDetail（其下 plan 行）。

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::pages::common::{container_status_color, mini_bar};
use crate::tui::panel::{render_panel, stack};

/// 某 milestone 下 plan 聚合的 issue 数（直属 issue 因 snapshot 无直属关系暂不计，见 #105 TODO）。
fn milestone_issue_count(m: &DashboardModel, mid: i64) -> usize {
    let plan_ids: Vec<i64> = m.milestone_plans(mid).iter().map(|(c, _)| c.id).collect();
    m.issues
        .iter()
        .filter(|i| i.plan_id.is_some_and(|p| plan_ids.contains(&p)))
        .count()
}

/// Milestones tab：全部 milestone 列表（每行状态点 + id + version + 标题 + plan/issue 数）。
pub fn draw_milestones_panel(frame: &mut Frame, m: &DashboardModel, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();
    for (idx, (ms, _)) in m.page_milestones().iter().enumerate() {
        let selected = idx == m.selected;
        let style = if selected {
            Style::new().add_modifier(Modifier::REVERSED)
        } else {
            Style::new()
        };
        let ver = ms.version.as_deref().unwrap_or("");
        let plan_count = m.milestone_plans(ms.id).len();
        let issue_count = milestone_issue_count(m, ms.id);
        lines.push(Line::from(vec![
            Span::styled("● ", container_status_color(ms.status)),
            Span::styled(format!("#{:<3}", ms.id), style),
            Span::styled(format!(" {ver:<8}"), style),
            Span::styled(format!("  {}", ms.title), style),
            Span::styled(format!("  plans {plan_count} · issues {issue_count}"), style),
        ]));
    }
    if lines.is_empty() {
        lines.push(Line::from("(no milestones)"));
    }
    let footer = format!(
        "j/k ↑↓ row · ←/→ page · 1/2/3 tab · Enter detail · q quit · Page {}/{}",
        m.page + 1,
        m.pages()
    );
    let chunks = stack(area, &[Constraint::Min(0), Constraint::Length(1)]);
    render_panel(frame, chunks[0], "milestones", lines);
    frame.render_widget(Paragraph::new(Line::from(footer)), chunks[1]);
}

/// MilestoneDetail：milestone 自身标题 + 其下 plan 行列表（每行迷你进度 + done/total）。
pub fn draw_milestone_detail(frame: &mut Frame, m: &DashboardModel, milestone_id: i64, area: Rect) {
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
            Span::styled(bar, style),
            Span::styled(format!(" {done}/{total}  {}", plan.title), style),
        ]));
    }
    if lines.is_empty() {
        lines.push(Line::from("(no plans in this milestone)"));
    }

    let footer = format!(
        "j/k ↑↓ plan · ←/→ page · 1/2/3 tab · Esc back · q quit · Page {}/{}",
        m.page + 1,
        m.pages()
    );
    let chunks = stack(area, &[Constraint::Min(0), Constraint::Length(1)]);
    render_panel(frame, chunks[0], &format!("milestone {title}"), lines);
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
    fn draw_milestones_panel_lists_milestones() {
        let mut m = model_full(
            vec![],
            vec![],
            vec![(mk_container(4, "TUI", Some("0.4.0"), None), 0)],
        );
        m.view = View::Milestones;
        let mut terminal = test_backend(60, 10);
        terminal
            .draw(|f| draw_milestones_panel(f, &m, f.area()))
            .unwrap();
        let text = buffer_text(terminal.backend().buffer()).join("\n");
        assert!(text.contains("milestones"), "标题: {text}");
        assert!(text.contains("0.4.0"), "version: {text}");
        assert!(text.contains("TUI"), "标题: {text}");
        assert!(text.contains("plans 0"), "plan 计数: {text}");
        assert!(text.contains("issues 0"), "issue 计数: {text}");
    }

    #[test]
    fn draw_milestone_detail_shows_plan_rows_with_progress() {
        let mut m = model_full(
            vec![
                mk_issue(1, "done one", Status::Done, Some(7)),
                mk_issue(2, "open one", Status::Open, Some(7)),
            ],
            vec![(mk_container(7, "tui plan", None, Some(4)), 0)],
            vec![(mk_container(4, "TUI", Some("0.4.0"), None), 0)],
        );
        m.view = View::MilestoneDetail { milestone_id: 4 };
        let mut terminal = test_backend(70, 10);
        terminal
            .draw(|f| draw_milestone_detail(f, &m, 4, f.area()))
            .unwrap();
        let text = buffer_text(terminal.backend().buffer()).join("\n");
        assert!(text.contains("milestone TUI (0.4.0)"), "标题: {text}");
        assert!(text.contains("tui plan"), "plan 标题: {text}");
        assert!(text.contains("1/2"), "进度: {text}");
    }
}
