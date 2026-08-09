//! Panel 组件：圆角容器（BorderType::Rounded）+ 标题 + 布局 helper。
//!
//! 页面绘制统一用此组件包裹内容，保证外观一致（紧贴外壁、带标题、圆角）。

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType, Paragraph};

/// 渲染圆角 panel：紧贴 area、带标题、内容多行。
pub fn render_panel(frame: &mut Frame, area: Rect, title: &str, lines: Vec<Line>) {
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .title(title),
        ),
        area,
    );
}

/// 纵向分栏（上下排布），按约束分配高度。
pub fn stack(area: Rect, constraints: &[Constraint]) -> Vec<Rect> {
    Layout::vertical(constraints.to_vec()).split(area).to_vec()
}

/// 横向分栏（左右排布），按约束分配宽度。
pub fn columns(area: Rect, constraints: &[Constraint]) -> Vec<Rect> {
    Layout::horizontal(constraints.to_vec())
        .split(area)
        .to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn buffer_text(buf: &ratatui::buffer::Buffer) -> Vec<String> {
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
    fn render_panel_draws_rounded_title_and_content() {
        let backend = TestBackend::new(40, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render_panel(f, f.area(), "hello", vec![Line::from("world")]);
            })
            .unwrap();
        let text = buffer_text(terminal.backend().buffer()).join("\n");
        assert!(text.contains("hello"), "标题: {text}");
        assert!(text.contains("world"), "内容: {text}");
        assert!(text.contains('╭'), "圆角边框: {text}");
    }

    #[test]
    fn stack_splits_vertically() {
        let area = Rect::new(0, 0, 40, 20);
        let rects = stack(
            area,
            &[Constraint::Percentage(50), Constraint::Percentage(50)],
        );
        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0].y, 0);
        assert_eq!(rects[1].y, 10);
    }

    #[test]
    fn columns_splits_horizontally() {
        let area = Rect::new(0, 0, 40, 20);
        let rects = columns(
            area,
            &[Constraint::Percentage(50), Constraint::Percentage(50)],
        );
        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0].x, 0);
        assert_eq!(rects[1].x, 20);
    }
}
