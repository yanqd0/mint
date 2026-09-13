//! plan 详情页：basic（键值对）+ body + kanban（6 态分列）+ issue list panel。

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::Color;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::models::{Issue, Status};
use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::pages::common::{
    body_lines_capped, container_status_color, kv_lines, panel_wrap,
};
use crate::tui::dashboard::pages::issues;
use crate::tui::panel::{columns, render_panel, render_panel_tight, stack};
use crate::tui::text::truncate;

/// kanban 面板总高（含 border）；内容区行数 = 面板高 - 2（#342 与 take 行数对齐）。
const KANBAN_PANEL_H: u16 = 10;

/// PlanDetail：basic / body / kanban / issue list 四个 panel。
pub fn draw_detail(frame: &mut Frame, m: &mut DashboardModel, plan_id: i64, area: Rect) {
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
    let mut kv: Vec<(String, Span<'static>)> = vec![
        (
            "status".into(),
            Span::styled(c.status.as_str(), container_status_color(c.status)),
        ),
        ("progress".into(), Span::raw(format!("{done}/{total}"))),
    ];
    if let Some(mid) = c.milestone_id {
        kv.push(("milestone".into(), Span::raw(format!("#{mid}"))));
    }
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

    // 布局：basic + body(≤10) + kanban(内容 8 行) + issues(弹性) + footer。
    // kanban 面板总高 KANBAN_PANEL_H，内容区 = 总高 - 2（border），取行数与之对齐（#342）。
    let mut constraints: Vec<Constraint> = vec![Constraint::Length(basic_rows.len() as u16 + 2)];
    if !body_lines.is_empty() {
        constraints.push(Constraint::Length(body_lines.len() as u16 + 2));
    }
    constraints.push(Constraint::Length(KANBAN_PANEL_H));
    constraints.push(Constraint::Min(0));
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

    // kanban panel（状态分列，ID+截断标题；dropped 空列隐藏省宽）。
    // 列合并（#348）：dev+test 都空合并为 `dev | test`；open+planned 都空且
    // dropped 有内容时合并为 `open | planned`。空列也显示（标题占位，dropped 除外）。
    let plan_issues: Vec<&Issue> = m.visible_issues();
    let has = |st: Status| plan_issues.iter().any(|i| i.status == st);
    // (标题前缀, 成员状态列表)：合并规则应用到相邻状态。
    let mut groups: Vec<(String, Vec<Status>)> = Vec::new();
    if !has(Status::Open) && !has(Status::Planned) && has(Status::Dropped) {
        groups.push(("open | planned".into(), vec![Status::Open, Status::Planned]));
    } else {
        groups.push(("open".into(), vec![Status::Open]));
        groups.push(("planned".into(), vec![Status::Planned]));
    }
    if !has(Status::Dev) && !has(Status::Test) {
        groups.push(("dev | test".into(), vec![Status::Dev, Status::Test]));
    } else {
        groups.push(("dev".into(), vec![Status::Dev]));
        groups.push(("test".into(), vec![Status::Test]));
    }
    groups.push(("done".into(), vec![Status::Done]));
    groups.push(("dropped".into(), vec![Status::Dropped]));

    let kanban_cols: Vec<(String, Vec<String>)> = groups
        .iter()
        .filter(|(_, statuses)| {
            // dropped 无 issue 时不显示该列（把宽度省出来）；其它组空列也显示。
            !(statuses.contains(&Status::Dropped) && statuses.iter().all(|s| !has(*s)))
        })
        .map(|(title, statuses)| {
            let items: Vec<&Issue> = plan_issues
                .iter()
                .copied()
                .filter(|i| statuses.contains(&i.status))
                .collect();
            // title 存完整，渲染时按列宽顶格（右侧）省略。
            // 取行数 = kanban 内容区行数（面板高 - 2 border）；溢出 `…` 占用末行（#342）。
            let avail = KANBAN_PANEL_H.saturating_sub(2) as usize;
            let mut rows: Vec<String> = items
                .iter()
                .take(avail.saturating_sub(1)) // 预留 `…` 行
                .map(|i| format!("#{} {}", i.id, i.title))
                .collect();
            if items.len() > avail.saturating_sub(1) {
                rows.push("…".into());
            }
            (format!("{title} ({})", items.len()), rows)
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
#[path = "plan_detail_tests.rs"]
mod tests;
