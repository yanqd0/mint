//! 共享渲染辅助：状态色 / 状态点 / 进度条（各页面复用）。

use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Padding, Paragraph, Wrap};

use crate::models::{ContainerStatus, Status};
use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::types::JumpKind;
use crate::tui::panel::panel_title;

/// 闪烁样式：目标（id+kind）在 `m.flash` 中 → SLOW_BLINK（列表行闪烁标记，变化内容提示）。
pub fn flash_style(m: &DashboardModel, id: i64, kind: JumpKind) -> Option<Style> {
    m.flash
        .iter()
        .find(|f| f.id == id && f.kind == kind)
        .map(|_| Style::new().add_modifier(Modifier::SLOW_BLINK))
}

/// 容器状态基色（milestone/plan 状态点用）。
pub fn container_status_color(status: ContainerStatus) -> Color {
    match status {
        ContainerStatus::Open => Color::Yellow,
        ContainerStatus::Running => Color::Green,
        ContainerStatus::Partial => Color::Cyan,
        ContainerStatus::Dropped => Color::Red,
        ContainerStatus::Done => Color::White,
    }
}

/// 状态基色（点/文字共用，TUI 统一配色）：
/// open 白 / planned·dev·test 黄（工作色）/ done 绿 / dropped 红。
fn status_color(status: Status) -> Color {
    match status {
        Status::Open => Color::White,
        Status::Planned | Status::Dev | Status::Test => Color::Yellow,
        Status::Done => Color::Green,
        Status::Dropped => Color::Red,
    }
}

/// 状态点是否默认闪烁（Planned 已排期、Dev 开发中、Test 测试中）。
fn status_blinks(status: Status) -> bool {
    matches!(status, Status::Planned | Status::Dev | Status::Test)
}

/// 状态点样式（闪烁状态加 SLOW_BLINK）。
fn status_style(status: Status) -> Style {
    let mut s = Style::new().fg(status_color(status));
    if status_blinks(status) {
        s = s.add_modifier(Modifier::SLOW_BLINK);
    }
    s
}

/// 状态点：`●` + 颜色（黄=待做、黄闪=已排期、绿闪=开发、绿=在做、白=完成、红=drop）。
pub fn status_dot(status: Status) -> (char, Style) {
    ('●', status_style(status))
}

/// 状态文字样式：与状态点同色但不闪烁。
pub fn status_text_style(status: Status) -> Style {
    Style::new().fg(status_color(status))
}

/// 状态简写（仅列表显示；planned→plan、dropped→drop，其余原样）。
pub fn status_abbrev(status: Status) -> &'static str {
    match status {
        Status::Planned => "plan",
        Status::Dropped => "drop",
        other => other.as_str(),
    }
}

/// kind 简写（仅列表显示；requirement→req、problem→bug）。
pub fn kind_abbrev(kind: crate::models::Kind) -> &'static str {
    match kind {
        crate::models::Kind::Requirement => "req",
        crate::models::Kind::Problem => "bug",
    }
}

/// 迷你进度条：固定宽度，█ 填充完成比例（milestone/plan 面板每行用），前后 padding。
pub fn mini_bar(done: usize, total: usize, width: usize) -> String {
    let body = if total == 0 {
        "░".repeat(width)
    } else {
        let filled = done.saturating_mul(width).checked_div(total).unwrap_or(0);
        "█".repeat(filled) + &"░".repeat(width - filled)
    };
    format!(" {body} ")
}

/// 计算 Table 弹性列（如 TITLE 的 Min/Fill）实际宽：区内宽 − 边框/内边距(4) − 列间距 − 定宽列合计。
/// 页面对该列内容预截断（右侧省略），避免长文本溢出/换行。
pub fn flex_col_width(area: Rect, widths: &[Constraint]) -> u16 {
    let fixed: u16 = widths
        .iter()
        .filter_map(|c| match c {
            Constraint::Length(n) => Some(*n),
            _ => None,
        })
        .sum();
    let spacing = widths.len().saturating_sub(1) as u16;
    area.width
        .saturating_sub(4) // border 2 + padding 2
        .saturating_sub(spacing)
        .saturating_sub(fixed)
}

/// 带标题的 wrap 段落：行超宽自动换行（不截断），basic panel 用。
/// 内容左右 1 格 padding（全局 margin 配置）；标题按面板宽自适应（见 `panel_title`）。
pub fn panel_wrap<'a>(title: &str, lines: Vec<Line<'a>>, width: u16) -> Paragraph<'a> {
    Paragraph::new(lines).wrap(Wrap { trim: true }).block(
        Block::bordered()
            .border_type(BorderType::Rounded)
            .title(panel_title(title, width.saturating_sub(2) as usize))
            .padding(Padding::horizontal(1)),
    )
}

/// body 段落：wrap 多行 + 圆角 border + 标题（issue/plan/milestone 详情 body panel 共用）。
/// 内容左右 1 格 padding（全局 margin 配置）；标题按面板宽自适应（见 `panel_title`）。
pub fn body_paragraph(body: &str, title: &str, width: u16) -> Paragraph<'static> {
    let lines: Vec<Line> = body
        .split('\n')
        .map(|l| Line::from(l.to_string()))
        .collect();
    Paragraph::new(lines).wrap(Wrap { trim: true }).block(
        Block::bordered()
            .border_type(BorderType::Rounded)
            .title(panel_title(title, width.saturating_sub(2) as usize))
            .padding(Padding::horizontal(1)),
    )
}

/// 键值对多列布局：**冒号对齐**（所有 kv 对 `key` 右对齐到全局最宽 key，`: ` 同列），
/// 列宽 = 该列最宽 + 2 padding。超宽 kv 对（单行放不下）退化为单列（每对一行），
/// value 由 panel wrap 续行（`key: ` 前缀保留首行，不单独续行悬空）。
/// 返回多行 `Line`，供详情页 basic panel 用（空值由调用方过滤后再传）。
pub fn kv_lines(pairs: &[(String, String)], width: u16) -> Vec<Line<'static>> {
    use unicode_width::UnicodeWidthStr;
    if pairs.is_empty() {
        return Vec::new();
    }
    let avail = width as usize;
    // 冒号对齐：key 右对齐到全局最宽 key。
    let max_key = pairs.iter().map(|(k, _)| k.width()).max().unwrap_or(0);
    let cells: Vec<(String, usize)> = pairs
        .iter()
        .map(|(k, v)| {
            let s = format!("{:<w$}: {v}", k, w = max_key);
            let w = s.width();
            (s, w)
        })
        .collect();
    // 超宽 kv 对 → 单列（每对一行，value wrap 续行）；否则按平均宽估列数。
    let any_wide = cells.iter().any(|(_, w)| *w > avail);
    let cols = if any_wide {
        1
    } else {
        let total_w: usize = cells.iter().map(|(_, w)| w).sum();
        let avg = total_w / cells.len();
        (avail / avg.max(1)).clamp(1, cells.len())
    };
    let rows = cells.len().div_ceil(cols);
    let mut col_widths = vec![0usize; cols];
    for (i, (_, w)) in cells.iter().enumerate() {
        col_widths[i % cols] = col_widths[i % cols].max(*w);
    }
    (0..rows)
        .map(|r| {
            let spans: Vec<Span> = (0..cols)
                .filter_map(|c| {
                    let idx = r * cols + c;
                    if idx >= cells.len() {
                        return None;
                    }
                    let (s, w) = &cells[idx];
                    let pad = col_widths[c] - w + 2; // 列间 2 空格
                    Some(Span::raw(format!("{s}{}", " ".repeat(pad))))
                })
                .collect();
            Line::from(spans)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_and_kind_abbrev() {
        use crate::models::Kind;
        assert_eq!(status_abbrev(Status::Planned), "plan");
        assert_eq!(status_abbrev(Status::Dropped), "drop");
        assert_eq!(status_abbrev(Status::Dev), "dev");
        assert_eq!(status_abbrev(Status::Test), "test");
        assert_eq!(kind_abbrev(Kind::Requirement), "req");
        assert_eq!(kind_abbrev(Kind::Problem), "bug");
    }

    #[test]
    fn status_dot_colors() {
        use ratatui::style::Color as C;
        assert_eq!(status_dot(Status::Open).0, '●');
        assert_eq!(status_dot(Status::Open).1.fg, Some(C::White));
        assert!(
            status_dot(Status::Planned)
                .1
                .add_modifier
                .contains(Modifier::SLOW_BLINK)
        );
        assert!(
            status_dot(Status::Dev)
                .1
                .add_modifier
                .contains(Modifier::SLOW_BLINK)
        );
        assert_eq!(status_dot(Status::Test).1.fg, Some(C::Yellow));
        assert!(
            status_dot(Status::Test)
                .1
                .add_modifier
                .contains(Modifier::SLOW_BLINK)
        );
        assert_eq!(status_dot(Status::Done).1.fg, Some(C::Green));
        assert_eq!(status_dot(Status::Dropped).1.fg, Some(C::Red));
    }

    #[test]
    fn status_text_style_same_color_no_blink() {
        use ratatui::style::Color as C;
        assert_eq!(status_text_style(Status::Dev).fg, Some(C::Yellow));
        assert!(
            !status_text_style(Status::Dev)
                .add_modifier
                .contains(Modifier::SLOW_BLINK)
        );
        assert_eq!(status_text_style(Status::Planned).fg, Some(C::Yellow));
        assert!(
            !status_text_style(Status::Planned)
                .add_modifier
                .contains(Modifier::SLOW_BLINK)
        );
        assert_eq!(status_text_style(Status::Done).fg, Some(C::Green));
    }

    #[test]
    fn flash_style_marks_target() {
        use crate::tui::dashboard::model::DashboardModel;
        use crate::tui::dashboard::types::FlashItem;
        let mut m = DashboardModel::new();
        m.flash = vec![FlashItem {
            id: 7,
            kind: JumpKind::Plan,
            ticks: 2,
        }];
        assert!(flash_style(&m, 7, JumpKind::Plan).is_some());
        assert!(flash_style(&m, 8, JumpKind::Plan).is_none());
    }

    #[test]
    fn flex_col_width_returns_remaining_after_fixed_and_spacing() {
        let widths = [
            Constraint::Length(18),
            Constraint::Length(22),
            Constraint::Length(11),
            Constraint::Min(0),
        ];
        // 区内宽 − 边框/内边距(4) − 列间距(3) − 定宽列(51)。
        assert_eq!(flex_col_width(Rect::new(0, 0, 100, 10), &widths), 42);
        assert_eq!(flex_col_width(Rect::new(0, 0, 70, 10), &widths), 12);
        // 窄到放不下 → 0（saturating）。
        assert_eq!(flex_col_width(Rect::new(0, 0, 10, 10), &widths), 0);
    }

    #[test]
    fn panel_wrap_title_ellipsis_when_narrow() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        let mut terminal = Terminal::new(TestBackend::new(30, 3)).unwrap();
        terminal
            .draw(|f| {
                f.render_widget(
                    panel_wrap(
                        "这是一个非常非常非常非常长的标题",
                        vec![Line::from("x")],
                        f.area().width,
                    ),
                    f.area(),
                );
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        let top: String = (0..buf.area.width).map(|x| buf[(x, 0)].symbol()).collect();
        // 标题按宽右侧省略，右角保留（不硬切角）。
        assert!(top.contains('…'), "窄宽下标题应省略: {top}");
        assert!(top.contains('╮'), "右角应保留: {top}");
    }

    #[test]
    fn mini_bar_fills_by_ratio_with_padding() {
        assert_eq!(mini_bar(0, 0, 4), " ░░░░ ");
        assert_eq!(mini_bar(1, 2, 4), " ██░░ ");
        assert_eq!(mini_bar(2, 2, 4), " ████ ");
        assert_eq!(mini_bar(0, 2, 4), " ░░░░ ");
    }

    #[test]
    fn kv_lines_single_row_when_wide() {
        let pairs = vec![
            ("status".to_string(), "planned".to_string()),
            ("kind".to_string(), "problem".to_string()),
            ("priority".to_string(), "0".to_string()),
        ];
        let lines = kv_lines(&pairs, 100);
        assert_eq!(lines.len(), 1);
        let text = lines[0]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<Vec<_>>()
            .join("");
        // 冒号对齐：key 右对齐到全局最宽 key（priority=8），status 补 2 空格。
        assert!(text.starts_with("status  : planned"), "冒号应对齐: {text}");
        assert!(text.contains("kind    : problem"), "冒号同列: {text}");
    }

    #[test]
    fn kv_lines_wraps_when_narrow() {
        let pairs = vec![
            ("status".to_string(), "planned".to_string()),
            ("kind".to_string(), "problem".to_string()),
        ];
        let lines = kv_lines(&pairs, 5);
        assert_eq!(lines.len(), 2); // 每行 1 列
        assert!(lines[0].spans[0].content.starts_with("status:"));
    }

    #[test]
    fn kv_lines_wide_pair_degrades_to_single_column() {
        let pairs = vec![
            ("status".to_string(), "planned".to_string()),
            (
                "body".to_string(),
                "a very very long body content that exceeds the width".to_string(),
            ),
        ];
        let lines = kv_lines(&pairs, 30);
        // 超宽 body 对 → 单列（每对独占一行，value 由 wrap 续行）。
        assert_eq!(lines.len(), 2);
        assert!(lines[0].spans[0].content.starts_with("status"));
        assert!(lines[1].spans[0].content.starts_with("body"));
    }
}
