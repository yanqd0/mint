//! plans 页面：milestone 分组 panel（执行中 milestone → 无 milestone → 剩余 milestone）。

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Cell, Padding, Paragraph, Row, Table};

use crate::models::Issue;
use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::pages::common::{container_status_color, flash_style, flex_col_width};
use crate::tui::dashboard::pages::progress::progress_bar;
use crate::tui::dashboard::types::JumpKind;
use crate::tui::panel::stack;
use crate::tui::text::truncate;

/// Plans tab：全部 plan 扁平列表（不按 milestone 分组），列：状态点 / ID / STATUS / PROGRESS / DONE/TOTAL / TITLE。
pub fn draw_plans_panel(frame: &mut Frame, m: &mut DashboardModel, area: Rect) {
    // 布局先定：表格面板高度（按可见高度分页）。
    let chunks = stack(area, &[Constraint::Min(0), Constraint::Length(1)]);
    let rows_avail = chunks[0].height.saturating_sub(3); // 边框 2 + 表头 1
    m.set_page_size(rows_avail as usize);

    // 列宽 + TITLE 弹性列实际宽（title 按此预截断、右侧省略）。
    let widths = [
        Constraint::Length(2),  // 状态点
        Constraint::Length(5),  // ID
        Constraint::Length(8),  // STATUS
        Constraint::Length(22), // PROGRESS
        Constraint::Length(10), // DONE/TOTAL（`{:>4}` 使 / 对齐 header）
        Constraint::Min(0),     // TITLE
    ];
    let title_w = flex_col_width(area, &widths);

    let header = Row::new(vec![
        Cell::from(""),
        Cell::from("ID"),
        Cell::from("STATUS"),
        Cell::from("PROGRESS"),
        Cell::from("DONE/TOTAL"), // 右对齐值用 {:>4} 使 / 与 header 对齐
        Cell::from("TITLE"),
    ])
    .style(Style::new().add_modifier(Modifier::BOLD));

    let mut rows: Vec<Row> = m
        .page_plans()
        .iter()
        .enumerate()
        .map(|(idx, (plan, _))| {
            let selected = m.selected_idx() == Some(idx);
            let (done, total) = m.plan_progress(plan.id);
            // PROGRESS 复用 progress_bar（plan 详情同款分段着色；无前导空格，与表头 P 对齐）。
            let plan_issues: Vec<&Issue> = m
                .issues
                .iter()
                .filter(|i| i.plan_id == Some(plan.id))
                .collect();
            let bar = progress_bar(&plan_issues, 20);
            let dot = container_status_color(plan.status);
            let mut row = Row::new(vec![
                Cell::from(Line::from(vec![Span::styled("●", Style::new().fg(dot))])),
                Cell::from(format!("#{}", plan.id)),
                Cell::from(Span::styled(
                    plan.status.as_str(),
                    container_status_color(plan.status),
                )),
                Cell::from(bar),
                Cell::from(format!("{:>4}/{}", done, total)), // / 固定在第 4 列，与 header DONE/TOTAL 对齐
                Cell::from(truncate(&plan.title, title_w.max(1) as usize)),
            ]);
            if selected {
                row = row.style(Style::new().add_modifier(Modifier::REVERSED));
            }
            if let Some(fs) = flash_style(m, plan.id, JumpKind::Plan) {
                row = row.style(fs);
            }
            row
        })
        .collect();
    if rows.is_empty() {
        rows.push(Row::new(vec![Cell::from("(no plans)")]));
    }

    let footer = format!(
        "j/k ↑↓ plan · ←/→ page · 1/2/3 tab · Enter detail · q quit · Page {}/{}",
        m.page + 1,
        m.pages()
    );
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
        buffer_text, cell_fg, mk_container, mk_issue, model_full, test_backend,
    };
    use crate::tui::dashboard::types::View;
    use ratatui::style::Color;

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
            .draw(|f| draw_plans_panel(f, &mut m, f.area()))
            .unwrap();
        let text = buffer_text(terminal.backend().buffer()).join("\n");
        assert!(text.contains("plans"), "标题: {text}");
        assert!(text.contains("ID"), "ID 表头: {text}");
        assert!(text.contains("STATUS"), "STATUS 表头: {text}");
        assert!(
            text.contains("running"),
            "plan 状态（mk_container 默认 Running）: {text}"
        );
        assert!(text.contains("tui plan"), "plan 标题: {text}");
        assert!(text.contains("1/1"), "进度: {text}");
        assert!(
            !text.contains("TUI (0.4.0)"),
            "应无 milestone 组标题: {text}"
        );
        // STATUS 列按容器状态色着色（running → 黄，与状态点一致）。
        assert_eq!(
            cell_fg(terminal.backend().buffer(), "running"),
            Some(Color::Yellow),
            "STATUS 列 running 应按容器状态色着色"
        );
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
            .draw(|f| draw_plans_panel(f, &mut m, f.area()))
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
            .draw(|f| draw_plans_panel(f, &mut m, f.area()))
            .unwrap();
        let text = buffer_text(terminal.backend().buffer()).join("\n");
        assert!(text.contains("free plan"), "plan 标题: {text}");
        assert!(!text.contains("no milestone"), "应无分组标题: {text}");
    }
}
