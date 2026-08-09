//! ratatui 渲染：只读分页表格 + footer。
//!
//! `draw` 供交互路径（`DefaultTerminal`）与非 TTY 降级（`TestBackend`）共用。

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph, Row, Table, TableState};
use unicode_width::UnicodeWidthStr;

use crate::tui::model::ListModel;

/// 渲染一帧：带标题边框的表格 + 选中行高亮 + footer 键位提示。
pub fn draw(frame: &mut Frame, m: &ListModel) {
    let block = Block::bordered().title(m.title.clone());
    let widths = column_widths(&m.headers, m.visible_rows());
    let header =
        Row::new(m.headers.iter().cloned()).style(Style::new().add_modifier(Modifier::BOLD));
    let rows: Vec<Row> = m
        .visible_rows()
        .iter()
        .map(|r| Row::new(r.iter().cloned()))
        .collect();
    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .row_highlight_style(Style::new().add_modifier(Modifier::REVERSED));
    let mut state = TableState::default();
    state.select(Some(m.selected));

    let chunks = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(frame.area());
    frame.render_stateful_widget(table, chunks[0], &mut state);
    let footer = format!(
        "Page {}/{}  ·  {} items  ·  j/k or ↑↓ move  h/l/PgUp/PgDn page  q quit",
        m.page,
        m.pages(),
        m.rows.len()
    );
    frame.render_widget(Paragraph::new(Line::from(footer)), chunks[1]);
}

/// 列宽：非末列 `Length(max(表头, 页内最大 cell)+2)`，末列 `Min` 弹性伸缩。
/// 纯函数：供 draw 与非 TTY 降级宽度计算复用。
pub fn column_widths(headers: &[String], rows: &[Vec<String>]) -> Vec<Constraint> {
    let n = headers.len();
    (0..n)
        .map(|c| {
            let max_cell = rows
                .iter()
                .map(|r| r.get(c).map_or(0, |s| UnicodeWidthStr::width(s.as_str())))
                .max()
                .unwrap_or(0);
            let w = max_cell.max(UnicodeWidthStr::width(headers[c].as_str())) as u16 + 2;
            if c == n - 1 {
                Constraint::Min(w)
            } else {
                Constraint::Length(w)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;

    fn model() -> ListModel {
        ListModel::new(
            "Issues",
            vec!["ID".into(), "Title".into()],
            vec![
                vec!["1".into(), "hello".into()],
                vec!["2".into(), "world".into()],
            ],
            5,
        )
    }

    fn buffer_lines(buf: &ratatui::buffer::Buffer) -> Vec<String> {
        (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn column_widths_follow_content() {
        let headers = vec!["ID".to_string(), "Title".to_string()];
        let rows = vec![
            vec!["1".to_string(), "short".to_string()],
            vec!["2".to_string(), "a much longer title".to_string()],
        ];
        let w = column_widths(&headers, &rows);
        assert_eq!(w[0], Constraint::Length(4)); // "ID"(2) + 2
        assert_eq!(w[1], Constraint::Min(21)); // 最长 cell(19) + 2
    }

    #[test]
    fn draw_renders_table_and_footer() {
        let m = model();
        let backend = TestBackend::new(60, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &m)).unwrap();
        let text = buffer_lines(terminal.backend().buffer());
        assert!(text.iter().any(|l| l.contains("Issues")), "标题: {text:?}");
        assert!(
            text.iter().any(|l| l.contains("ID") && l.contains("Title")),
            "表头: {text:?}"
        );
        assert!(text.iter().any(|l| l.contains("hello")), "数据行: {text:?}");
        assert!(
            text.iter().any(|l| l.contains("Page 1/1")),
            "footer: {text:?}"
        );
    }

    #[test]
    fn draw_empty_rows_renders_footer() {
        let m = ListModel::new("Empty", vec!["ID".into()], vec![], 5);
        let backend = TestBackend::new(30, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &m)).unwrap();
        let text = buffer_lines(terminal.backend().buffer());
        assert!(
            text.iter().any(|l| l.contains("Page 1/1")),
            "footer: {text:?}"
        );
    }
}
