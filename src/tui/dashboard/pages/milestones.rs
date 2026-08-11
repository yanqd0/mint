//! milestones 页面：Milestones tab 列表 + MilestoneDetail（其下 plan 行）。

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Cell, Padding, Paragraph, Row, Table};

use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::pages::common::{
    container_status_color, flash_style, flex_col_width, mini_bar,
};
use crate::tui::dashboard::types::JumpKind;
use crate::tui::panel::{render_panel, stack};
use crate::tui::text::truncate;

/// 某 milestone 下 issue 数：返回 (总数, 直属数)。总数 = plan 聚合 issue + 直属 issue。
fn milestone_counts(m: &DashboardModel, mid: i64) -> (usize, usize) {
    let plan_ids: Vec<i64> = m.milestone_plans(mid).iter().map(|(c, _)| c.id).collect();
    let plan_issue = m
        .issues
        .iter()
        .filter(|i| i.plan_id.is_some_and(|p| plan_ids.contains(&p)))
        .count();
    let direct = m.milestone_direct_ids(mid).len();
    (plan_issue + direct, direct)
}

/// Milestones tab：全部 milestone 列表（ratatui Table：状态点 + id + version + PLANS/ISSUES 数字列 + TITLE 右置）。
pub fn draw_milestones_panel(frame: &mut Frame, m: &mut DashboardModel, area: Rect) {
    // 布局先定：表格面板高度（按可见高度分页）。
    let chunks = stack(area, &[Constraint::Min(0), Constraint::Length(1)]);
    let rows_avail = chunks[0].height.saturating_sub(3); // 边框 2 + 表头 1
    m.set_page_size(rows_avail as usize);
    // 列宽 + TITLE 弹性列实际宽（title 按此预截断、右侧省略，避免长文本溢出/换行）。
    let widths = [
        Constraint::Length(2),
        Constraint::Length(5),
        Constraint::Length(8), // STATUS
        Constraint::Length(9),
        Constraint::Length(6),
        Constraint::Length(10),
        Constraint::Min(0),
    ];
    let title_w = flex_col_width(area, &widths);

    let header = Row::new(vec![
        "", "#", "STATUS", "VERSION", "PLANS", "ISSUES", "TITLE",
    ])
    .style(Style::new().add_modifier(Modifier::BOLD));
    let rows: Vec<Row> = m
        .page_milestones()
        .iter()
        .enumerate()
        .map(|(idx, (ms, _))| {
            let selected = m.selected_idx() == Some(idx);
            let ver = ms.version.clone().unwrap_or_default();
            let plan_count = m.milestone_plans(ms.id).len();
            let (total, direct) = milestone_counts(m, ms.id);
            let mut row = Row::new(vec![
                Cell::from(Line::from(vec![Span::styled(
                    "●",
                    Style::new().fg(container_status_color(ms.status)),
                )])),
                Cell::from(format!("#{}", ms.id)),
                Cell::from(Span::styled(
                    ms.status.as_str(),
                    container_status_color(ms.status),
                )),
                Cell::from(ver),
                Cell::from(plan_count.to_string()),
                Cell::from(format!("{total}({direct})")), // 总数(直属)
                Cell::from(truncate(&ms.title, title_w.max(1) as usize)),
            ]);
            if selected {
                row = row.style(Style::new().add_modifier(Modifier::REVERSED));
            }
            if let Some(fs) = flash_style(m, ms.id, JumpKind::Milestone) {
                row = row.style(fs);
            }
            row
        })
        .collect();
    let footer = format!(
        "j/k ↑↓ row · ←/→ page · 1/2/3 tab · Enter detail · q quit · Page {}/{}",
        m.page + 1,
        m.pages()
    );
    let title = format!("─milestones · page {}/{}", m.page + 1, m.pages());
    let mut rows = rows;
    if rows.is_empty() {
        rows.push(Row::new(vec![Cell::from("(no milestones)")]));
    }
    let table = Table::new(rows, widths).header(header).block(
        Block::bordered()
            .title(title)
            .padding(Padding::horizontal(1)),
    );
    frame.render_widget(table, chunks[0]);
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
        let selected = m.selected_idx() == Some(idx);
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
        buffer_text, cell_fg, mk_container, mk_issue, model_full, test_backend,
    };
    use crate::tui::dashboard::types::View;
    use ratatui::style::Color;

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
            .draw(|f| draw_milestones_panel(f, &mut m, f.area()))
            .unwrap();
        let text = buffer_text(terminal.backend().buffer()).join("\n");
        assert!(text.contains("milestones"), "标题: {text}");
        assert!(text.contains("0.4.0"), "version: {text}");
        assert!(text.contains("TUI"), "标题: {text}");
        assert!(text.contains("PLANS"), "PLANS 表头: {text}");
        assert!(text.contains("0(0)"), "ISSUES 总数(直属): {text}");
        // STATUS 列按容器状态色着色（running → 黄，与状态点一致）。
        assert_eq!(
            cell_fg(terminal.backend().buffer(), "running"),
            Some(Color::Yellow),
            "STATUS 列 running 应按容器状态色着色"
        );
    }

    #[test]
    fn milestone_title_truncates_with_ellipsis_when_long() {
        let mut m = model_full(
            vec![],
            vec![],
            vec![(
                mk_container(
                    4,
                    "一个非常非常非常非常非常长的 milestone 标题用于验证截断省略行为",
                    Some("0.4.0"),
                    None,
                ),
                0,
            )],
        );
        m.view = View::Milestones;
        let mut terminal = test_backend(100, 10);
        terminal
            .draw(|f| draw_milestones_panel(f, &mut m, f.area()))
            .unwrap();
        let lines = buffer_text(terminal.backend().buffer());
        let row = lines
            .iter()
            .find(|l| l.contains("#4"))
            .expect("milestone 行");
        assert!(row.contains('…'), "长 milestone 标题应右侧省略: {row}");
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
