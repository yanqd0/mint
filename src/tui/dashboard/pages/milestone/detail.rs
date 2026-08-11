//! milestone 详情页：自身信息 + 其下 plan 列表 + issue 聚合（三 panel）。

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::models::{Issue, Status};
use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::pages::common::{
    body_lines_capped, container_status_color, kv_lines, mini_bar, panel_wrap, status_dot,
};
use crate::tui::dashboard::pages::progress::{progress_bar, progress_pct_line};
use crate::tui::panel::{render_panel, stack};
use crate::tui::text::truncate;

/// MilestoneDetail：basic / body / plan 列表 / 直属 issue 列表 四个 panel。
pub fn draw_detail(frame: &mut Frame, m: &mut DashboardModel, milestone_id: i64, area: Rect) {
    let Some((c, _)) = m.milestones.iter().find(|(c, _)| c.id == milestone_id) else {
        render_panel(
            frame,
            area,
            "milestone",
            vec![Line::from(format!("#{milestone_id} (deleted)"))],
        );
        return;
    };
    // 进度计数（临时 scope_issues，不跨后续 page_size mutation 持有借用）。
    let total = m.scope_issues().len();
    let done = m
        .scope_issues()
        .iter()
        .filter(|i| matches!(i.status, Status::Done | Status::Dropped))
        .count();

    // 1. basic 键值对（有值才显）。
    let mut kv: Vec<(String, Span<'static>)> = vec![
        (
            "status".into(),
            Span::styled(c.status.as_str(), container_status_color(c.status)),
        ),
        ("progress".into(), Span::raw(format!("{done}/{total}"))),
    ];
    if let Some(v) = &c.version {
        kv.push(("version".into(), Span::raw(v.clone())));
    }
    kv.push((
        "created".into(),
        Span::styled(c.created_at.clone(), Color::Magenta),
    ));
    kv.push((
        "updated".into(),
        Span::styled(c.updated_at.clone(), Color::Magenta),
    ));
    let basic_rows = kv_lines(&kv, area.width.saturating_sub(4));

    // body ≤10 行（多余省略，遇 \n 换行 + 贪心 word-wrap）。
    let body_lines: Vec<Line> = c
        .body
        .as_ref()
        .map(|b| {
            body_lines_capped(b, area.width.saturating_sub(4) as usize, 10)
                .into_iter()
                .map(Line::from)
                .collect()
        })
        .unwrap_or_default();

    // 2. plans panel（内容定高）+ issues panel（填满剩余），各自独立分页；跨 panel 导航保留，selected 1-indexed 跨段。
    // 剩余可用高度（basic + body? + progress(4) + footer(1) 之外），plans 页大小取一半（给 issues 留空间）。
    let avail_h = area
        .height
        .saturating_sub(basic_rows.len() as u16 + 2)
        .saturating_sub(if body_lines.is_empty() {
            0
        } else {
            body_lines.len() as u16 + 2
        })
        .saturating_sub(4)
        .saturating_sub(1);
    m.plans_page_size = (avail_h as usize / 2).max(1);
    let plans = m.page_milestone_plans(milestone_id);
    let n = plans.len();
    let mut plan_lines: Vec<Line> = Vec::new();
    if plans.is_empty() {
        plan_lines.push(Line::from("(no plans in this milestone)"));
    }
    for (i, (plan, _)) in plans.iter().enumerate() {
        let (pdone, ptotal) = m.plan_progress(plan.id);
        let bar = mini_bar(pdone, ptotal, 20);
        let sel = m.selected_idx() == Some(i); // plans 段：selected 1..=n
        let style = if sel {
            Style::new().add_modifier(Modifier::REVERSED)
        } else {
            Style::new()
        };
        // 前缀（状态点+id+bar+进度）宽度固定，title 按面板内容宽 − 前缀宽截断（右侧省略），避免溢出。
        let dot = container_status_color(plan.status);
        let prefix = format!("#{:<3}{bar} {pdone}/{ptotal}  ", plan.id);
        let avail = area.width.saturating_sub(4) as usize; // render_panel 内容宽（border 2 + padding 2）
        let pw = unicode_width::UnicodeWidthStr::width(prefix.as_str()) + 2; // + ● 空格
        plan_lines.push(Line::from(vec![
            Span::styled("● ", Style::new().fg(dot)),
            Span::styled(prefix, style),
            Span::styled(truncate(&plan.title, avail.saturating_sub(pw)), style),
        ]));
    }
    // plans 面板内容定高后，issues 填满剩余、按该高度分页。
    let plans_panel_h = plan_lines.len() as u16 + 2;
    m.issues_page_size = (avail_h as usize)
        .saturating_sub(plans_panel_h as usize)
        .saturating_sub(2)
        .max(1);
    let page_issues = m.page_milestone_issues(milestone_id);
    // 进度条数据（直属+间接全部 issue；mutation 完成后再取，避免借用冲突）。
    let all: Vec<&Issue> = m.scope_issues();
    let mut issue_lines: Vec<Line> = Vec::new();
    if page_issues.is_empty() {
        issue_lines.push(Line::from("(no issues in this milestone)"));
    }
    for (j, issue) in page_issues.iter().enumerate() {
        let sel = m.selected_idx() == Some(n + j); // issues 段：selected n+1..
        let style = if sel {
            Style::new().add_modifier(Modifier::REVERSED)
        } else {
            Style::new()
        };
        // 前缀（状态点+id）固定，title 按剩余宽截断，避免溢出。
        let (dot, dot_style) = status_dot(issue.status);
        let id_part = format!("#{} ", issue.id);
        let avail = area.width.saturating_sub(4) as usize;
        let pw = 2 + unicode_width::UnicodeWidthStr::width(id_part.as_str());
        issue_lines.push(Line::from(vec![
            Span::styled(format!("{dot} "), dot_style),
            Span::styled(id_part, style),
            Span::styled(truncate(&issue.title, avail.saturating_sub(pw)), style),
        ]));
    }

    // 布局：basic + body(≤10) + progress + plans（内容定高）+ issues（填满剩余）+ footer。
    let mut constraints: Vec<Constraint> = vec![Constraint::Length(basic_rows.len() as u16 + 2)];
    if !body_lines.is_empty() {
        constraints.push(Constraint::Length(body_lines.len() as u16 + 2));
    }
    constraints.push(Constraint::Length(4)); // progress 面板（bar + 分组百分比）
    constraints.push(Constraint::Length(plans_panel_h));
    constraints.push(Constraint::Min(0)); // issues 填满剩余
    constraints.push(Constraint::Length(1));
    let chunks = stack(area, &constraints);
    let mut ci = 0;

    frame.render_widget(
        panel_wrap(
            &format!("#{} {}", c.id, c.title),
            basic_rows,
            chunks[ci].width,
        ),
        chunks[ci],
    );
    ci += 1;
    if !body_lines.is_empty() {
        render_panel(frame, chunks[ci], "body", body_lines);
        ci += 1;
    }
    // progress 面板：直接+间接全部 issue 聚合进度条（dropped 红色计入完成）。
    let bw = chunks[ci].width.saturating_sub(4) as usize; // render_panel 内容宽（border 2 + padding 2）
    render_panel(
        frame,
        chunks[ci],
        "progress",
        vec![progress_bar(&all, bw), progress_pct_line(&all)],
    );
    ci += 1;
    render_panel(
        frame,
        chunks[ci],
        &format!(
            "plans · page {}/{}",
            m.plans_page + 1,
            m.milestone_plans_pages(milestone_id)
        ),
        plan_lines,
    );
    ci += 1;
    render_panel(
        frame,
        chunks[ci],
        &format!(
            "issues · page {}/{}",
            m.issues_page + 1,
            m.milestone_issues_pages(milestone_id)
        ),
        issue_lines,
    );
    ci += 1;
    frame.render_widget(
        Paragraph::new(Line::from(
            "j/k ↑↓ row · ←/→ page · 1/2/3 tab · Esc back · q quit",
        )),
        chunks[ci],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Container;
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
        terminal
            .draw(|f| draw_detail(f, &mut m, 4, f.area()))
            .unwrap();
        let text = buffer_text(terminal.backend().buffer()).join("\n");
        assert!(text.contains("#4 TUI"), "标题: {text}");
        assert!(text.contains("0.4.0"), "version: {text}");
        assert!(text.contains("plans"), "plan 列表标题: {text}");
        assert!(text.contains("tui plan"), "plan 行: {text}");
        assert!(text.contains("╭─issues"), "issues panel: {text}");
        assert!(text.contains("open one"), "直属 issue 行: {text}");
    }

    #[test]
    fn progress_panel_shows_aggregate_direct_and_indirect() {
        // plan 7 属 ms4（1 个 done 间接）+ issue 2 直属 ms4（dropped）。
        let mut m = model_full(
            vec![
                mk_issue(1, "done in plan", Status::Done, Some(7)),
                mk_issue(2, "direct dropped", Status::Dropped, None),
            ],
            vec![(mk_container(7, "tui plan", None, Some(4)), 0)],
            vec![(mk_container(4, "TUI", Some("0.4.0"), None), 0)],
        );
        m.milestone_directs = vec![(4, 2)];
        m.view = View::MilestoneDetail { milestone_id: 4 };
        let mut terminal = test_backend(80, 20);
        terminal
            .draw(|f| draw_detail(f, &mut m, 4, f.area()))
            .unwrap();
        let text = buffer_text(terminal.backend().buffer()).join("\n");
        // 直接+间接 2 issue（done + dropped）→ 分组行含 dropped 50%。
        assert!(text.contains("dropped 50%"), "聚合进度: {text}");
    }

    #[test]
    fn milestone_detail_shows_page_numbers_when_paged() {
        let plans: Vec<(Container, i64)> = (1..=12)
            .map(|i| (mk_container(i, &format!("plan {i}"), None, Some(4)), 0))
            .collect();
        let mut m = model_full(
            vec![],
            plans,
            vec![(mk_container(4, "TUI", Some("0.4.0"), None), 0)],
        );
        m.view = View::MilestoneDetail { milestone_id: 4 };
        let mut terminal = test_backend(100, 30);
        terminal
            .draw(|f| draw_detail(f, &mut m, 4, f.area()))
            .unwrap();
        let text = buffer_text(terminal.backend().buffer()).join("\n");
        assert!(text.contains("plans · page 1/2"), "plans 页码: {text}");
        assert!(text.contains("issues · page 1/1"), "issues 页码: {text}");
    }

    #[test]
    fn plan_row_title_truncates_when_long() {
        let mut m = model_full(
            vec![mk_issue(1, "a", Status::Done, Some(7))],
            vec![(
                mk_container(
                    7,
                    "一个非常非常非常非常非常长的 plan 标题用于验证省略",
                    None,
                    Some(4),
                ),
                0,
            )],
            vec![(mk_container(4, "TUI", Some("0.4.0"), None), 0)],
        );
        m.view = View::MilestoneDetail { milestone_id: 4 };
        let mut terminal = test_backend(80, 20);
        terminal
            .draw(|f| draw_detail(f, &mut m, 4, f.area()))
            .unwrap();
        let lines = buffer_text(terminal.backend().buffer());
        let plan_row = lines.iter().find(|l| l.contains("#7")).expect("plan 行");
        assert!(plan_row.contains('…'), "长 plan 标题应右侧省略: {plan_row}");
    }

    #[test]
    fn milestone_body_truncates_to_10_lines() {
        let mut m = model_full(
            vec![mk_issue(1, "open one", Status::Open, Some(7))],
            vec![(mk_container(7, "tui plan", None, Some(4)), 0)],
            vec![(mk_container(4, "TUI", Some("0.4.0"), None), 0)],
        );
        m.milestones[0].0.body = Some(
            (1..=20)
                .map(|i| format!("line {i}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        m.view = View::MilestoneDetail { milestone_id: 4 };
        let mut terminal = test_backend(100, 40);
        terminal
            .draw(|f| draw_detail(f, &mut m, 4, f.area()))
            .unwrap();
        let text = buffer_text(terminal.backend().buffer()).join("\n");
        assert!(text.contains("line 1"), "body 开头: {text}");
        assert!(text.contains("line 10…"), "末行省略: {text}");
        assert!(!text.contains("line 11"), "超限行省略: {text}");
    }

    #[test]
    fn milestone_detail_omits_issues_panel_without_any_issue() {
        let mut m = model_full(
            vec![],
            vec![(mk_container(7, "tui plan", None, Some(4)), 0)],
            vec![(mk_container(4, "TUI", Some("0.4.0"), None), 0)],
        );
        let mut terminal = test_backend(100, 20);
        terminal
            .draw(|f| draw_detail(f, &mut m, 4, f.area()))
            .unwrap();
        let text = buffer_text(terminal.backend().buffer()).join("\n");
        assert!(
            text.contains("no issues in this milestone"),
            "无任何 issue 时 issues panel 显示空提示: {text}"
        );
    }
}
