//! milestone 详情页：自身信息 + 其下 plan 列表 + issue 聚合（三 panel）。

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::models::{Issue, Status};
use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::pages::common::{
    body_lines_capped, container_status_color, kv_lines, list_title, panel_wrap, status_dot,
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
        let sel = m.selected_idx() == Some(i); // plans 段：selected 1..=n
        let style = if sel {
            Style::new().add_modifier(Modifier::REVERSED)
        } else {
            Style::new()
        };
        // 进度条复用 progress_bar（彩色 4 色分组 + dropped 红段），与 plans 面板一致；
        // 选中行 bar 补 REVERSED（保持配色 + 高亮），对齐 plans 面板行级 REVERSED。
        let plan_issues: Vec<&Issue> = m
            .issues
            .iter()
            .filter(|i| i.plan_id == Some(plan.id))
            .collect();
        let mut bar = progress_bar(&plan_issues, 20);
        if sel {
            for sp in bar.spans.iter_mut() {
                sp.style = sp.style.add_modifier(Modifier::REVERSED);
            }
        }
        // 前缀（状态点+id+bar+进度）宽度固定，title 按面板内容宽 − 前缀宽截断（右侧省略），避免溢出。
        let dot = container_status_color(plan.status);
        let id_part = format!("#{:<3}", plan.id);
        let tail = format!("  {pdone}/{ptotal}  "); // bar 后空格 + 进度文本
        let avail = area.width.saturating_sub(4) as usize; // render_panel 内容宽（border 2 + padding 2）
        let pw = 2
            + unicode_width::UnicodeWidthStr::width(id_part.as_str())
            + 1
            + 20
            + unicode_width::UnicodeWidthStr::width(tail.as_str()); // ●空格 + id + bar前空格 + bar + tail
        let mut spans: Vec<Span> = vec![
            Span::styled("● ", Style::new().fg(dot)),
            Span::styled(id_part, style),
            Span::styled(" ", style), // bar 前空格
        ];
        spans.extend(bar.spans);
        spans.push(Span::styled(tail, style));
        spans.push(Span::styled(
            truncate(&plan.title, avail.saturating_sub(pw)),
            style,
        ));
        plan_lines.push(Line::from(spans));
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
        &list_title(
            "plans",
            m.plans_page + 1,
            m.milestone_plans_pages(milestone_id),
            n,
            m.milestone_plans(milestone_id).len(),
        ),
        plan_lines,
    );
    ci += 1;
    render_panel(
        frame,
        chunks[ci],
        &list_title(
            "issues",
            m.issues_page + 1,
            m.milestone_issues_pages(milestone_id),
            page_issues.len(),
            m.scope_issues().len(),
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
#[path = "detail_tests.rs"]
mod tests;
