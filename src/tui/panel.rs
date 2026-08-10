//! Panel 组件：圆角容器（BorderType::Rounded）+ 标题 + 布局 helper。
//!
//! 页面绘制统一用此组件包裹内容，保证外观一致（紧贴外壁、带标题、圆角）。

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType, Padding, Paragraph};

use crate::tui::text::truncate;

/// 渲染圆角 panel：紧贴 area、带标题、内容多行。内容左右 1 格 padding（全局 margin 配置）。
pub fn render_panel(frame: &mut Frame, area: Rect, title: &str, lines: Vec<Line>) {
    render_panel_with(frame, area, title, lines, Padding::horizontal(1));
}

/// 渲染圆角 panel（无内容 padding）：窄列场景（kanban 6 态列等）省宽度。
pub fn render_panel_tight(frame: &mut Frame, area: Rect, title: &str, lines: Vec<Line>) {
    render_panel_with(frame, area, title, lines, Padding::ZERO);
}

/// panel 标题串：`title 宽 + 1 ≤ avail` 时带 `─` 前缀（标题右移一格），
/// 否则去前缀仅顶格右侧省略。避免 Block title 超宽硬切角（窄列/长标题，宽可随 resize 改变）。
pub fn panel_title(title: &str, avail: usize) -> String {
    use unicode_width::UnicodeWidthStr;
    if title.is_empty() || avail == 0 {
        return String::new();
    }
    // `─{title}` 宽 = 1 + title 宽；有空间才加前缀（avail≥1 已由上文保证）。
    if title.width() < avail {
        format!("─{title}")
    } else {
        truncate(title, avail)
    }
}

/// panel 渲染核心：统一圆角 border + 标题，padding 由调用方定。
/// 标题按面板宽自适应（见 `panel_title`），content 不截断（wrap 由调用方控制）。
fn render_panel_with(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    lines: Vec<Line>,
    padding: Padding,
) {
    let titled = panel_title(title, area.width.saturating_sub(2) as usize);
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .title(titled)
                .padding(padding),
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
    fn panel_title_fits_with_prefix_else_right_ellipsis() {
        // 有空间放 ─ 前缀：带前缀完整显示。
        assert_eq!(panel_title("open (1)", 10), "─open (1)");
        // 无空间放前缀但标题本身放得下：去前缀完整显示（不硬切角）。
        assert_eq!(panel_title("open (1)", 8), "open (1)");
        // 超宽：顶格右侧省略。
        assert_eq!(panel_title("planned (0)", 8), "planned…");
        // 空 / 0 宽。
        assert_eq!(panel_title("", 10), "");
        assert_eq!(panel_title("open (1)", 0), "");
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
