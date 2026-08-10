//! plans 页面：milestone 分组 panel（执行中 milestone → 无 milestone → 剩余 milestone）。

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Cell, Padding, Paragraph, Row, Table};

use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::pages::common::{flash_style, flex_col_width, mini_bar};
use crate::tui::dashboard::types::JumpKind;
use crate::tui::panel::stack;
use crate::tui::text::truncate;

/// Plans tab：按 milestone 分组渲染（每组一 panel 标题），行选中按展平行号映射。
pub fn draw_plans_panel(frame: &mut Frame, m: &DashboardModel, area: Rect) {
    let groups = m.plan_groups();
    let start = m.page * m.page_size;
    let end = start + m.page_size;
    // 列宽 + TITLE 弹性列实际宽（title 按此预截断、右侧省略，避免长文本溢出/换行）。
    let widths = [
        Constraint::Length(18),
        Constraint::Length(22),
        Constraint::Length(11),
        Constraint::Min(0),
    ];
    let title_w = flex_col_width(area, &widths);

    let header = Row::new(vec!["#", "PROGRESS", "DONE/TOTAL", "TITLE"])
        .style(Style::new().add_modifier(Modifier::BOLD));
    let mut rows: Vec<Row> = Vec::new();
    let mut n = 0; // 展平行号（全局，仅计 plan 行）
    for g in &groups {
        let g_start = n;
        let g_end = n + g.plans.len();
        n = g_end;
        // 组标题仅在页内有该组行时显示（跨列，其余列空）。
        if g_end <= start || g_start >= end {
            continue;
        }
        // 组标题按首列宽（18）截断，避免溢出。
        rows.push(
            Row::new(vec![truncate(&g.title, 18)]).style(Style::new().add_modifier(Modifier::BOLD)),
        );
        for (i, (plan, _)) in g.plans.iter().enumerate() {
            let global = g_start + i;
            if global < start || global >= end {
                continue;
            }
            let selected = m.selected_idx() == Some(global - start);
            let (done, total) = m.plan_progress(plan.id);
            let bar = mini_bar(done, total, 20);
            let mut row = Row::new(vec![
                Cell::from(format!("#{}", plan.id)),
                Cell::from(bar),
                Cell::from(format!("{done}/{total}")),
                Cell::from(truncate(&plan.title, title_w.max(1) as usize)),
            ]);
            if selected {
                row = row.style(Style::new().add_modifier(Modifier::REVERSED));
            }
            if let Some(fs) = flash_style(m, plan.id, JumpKind::Plan) {
                row = row.style(fs);
            }
            rows.push(row);
        }
    }
    if rows.is_empty() {
        rows.push(Row::new(vec![Cell::from("(no plans)")]));
    }

    let footer = format!(
        "j/k ↑↓ plan · ←/→ page · 1/2/3 tab · Enter detail · q quit · Page {}/{}",
        m.page + 1,
        m.pages()
    );
    let chunks = stack(area, &[Constraint::Min(0), Constraint::Length(1)]);
    let title = format!("─plans · page {}/{}", m.page + 1, m.pages());
    let table = Table::new(rows, widths).header(header).block(
        Block::bordered()
            .title(title)
            .padding(Padding::horizontal(1)),
    );
    frame.render_widget(table, chunks[0]);
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
    fn plan_title_truncates_with_ellipsis_when_long() {
        let mut m = model_full(
            vec![mk_issue(1, "a", Status::Done, Some(7))],
            vec![(
                mk_container(
                    7,
                    "一个非常非常非常非常非常长的 plan 标题用于验证截断省略行为",
                    None,
                    Some(4),
                ),
                0,
            )],
            vec![(mk_container(4, "TUI", Some("0.4.0"), None), 0)],
        );
        m.view = View::Plans;
        let mut terminal = test_backend(100, 12);
        terminal
            .draw(|f| draw_plans_panel(f, &m, f.area()))
            .unwrap();
        let lines = buffer_text(terminal.backend().buffer());
        let row = lines.iter().find(|l| l.contains("#7")).expect("plan 行");
        assert!(row.contains('…'), "长 plan 标题应右侧省略: {row}");
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
