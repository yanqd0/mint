//! common.rs 拆分的独立测试模块。

use super::*;

#[test]
fn label_style_bg_and_contrast_fg() {
    // 暗背景（#1f6feb 蓝）→ 背景=记录色，前景=白。
    let s = label_style("#1f6feb");
    assert_eq!(
        s,
        Style::new()
            .bg(Color::Rgb(0x1f, 0x6f, 0xeb))
            .fg(Color::White)
    );
    // 亮背景（#bbdd3c 黄绿）→ 前景=黑。
    let bright = label_style("#bbdd3c");
    assert_eq!(
        bright,
        Style::new()
            .bg(Color::Rgb(0xbb, 0xdd, 0x3c))
            .fg(Color::Black)
    );
    // 空串/非法 → 默认样式。
    assert_eq!(label_style(""), Style::default());
    assert_eq!(label_style("not-a-color"), Style::default());
    assert_eq!(label_style("#fff"), Style::default());
}

#[test]
fn status_and_kind_abbrev() {
    use crate::models::Kind;
    assert_eq!(status_abbrev(Status::Planned), "plan");
    assert_eq!(status_abbrev(Status::Dropped), "drop");
    assert_eq!(status_abbrev(Status::Dev), "dev");
    assert_eq!(status_abbrev(Status::Test), "test");
    assert_eq!(kind_abbrev(Kind::Requirement), "req");
    assert_eq!(kind_abbrev(Kind::Problem), "bug");
    assert_eq!(kind_abbrev(Kind::Task), "task");
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
fn body_lines_capped_respects_newlines_and_wrap() {
    let body = "line one\nline two\nthird line";
    let lines = body_lines_capped(body, 100, 10);
    // 显式 \n 换行优先于 word-wrap：3 段各 1 行。
    assert_eq!(lines, vec!["line one", "line two", "third line"]);
}

#[test]
fn body_lines_capped_caps_at_max_lines_with_ellipsis() {
    let body = (1..=15)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let lines = body_lines_capped(&body, 100, 10);
    assert_eq!(lines.len(), 10, "超限截到 max_lines");
    assert_eq!(lines[9], "line 10…", "末行加省略号");
}

#[test]
fn body_lines_capped_wraps_long_lines_then_caps() {
    // 单行长文本 + 窄宽 → 先 wrap 再封顶。
    let body = "one two three four five six seven eight nine ten";
    let lines = body_lines_capped(body, 10, 5);
    assert_eq!(lines.len(), 5, "窄宽 wrap 后仍封顶");
    assert!(lines[4].ends_with('…'), "末行应带省略号");
}

#[test]
fn kv_lines_single_row_when_wide() {
    let pairs = vec![
        ("status".to_string(), Span::raw("planned")),
        ("kind".to_string(), Span::raw("problem")),
        ("priority".to_string(), Span::raw("0")),
    ];
    let lines = kv_lines(&pairs, 100);
    assert_eq!(lines.len(), 1);
    let text = lines[0]
        .spans
        .iter()
        .map(|s| s.content.as_ref())
        .collect::<Vec<_>>()
        .join("");
    // 紧凑 key: value | 分隔（无冒号对齐）。
    assert_eq!(text, "status: planned | kind: problem | priority: 0");
}

#[test]
fn kv_lines_wraps_when_narrow() {
    let pairs = vec![
        ("status".to_string(), Span::raw("planned")),
        ("kind".to_string(), Span::raw("problem")),
    ];
    let lines = kv_lines(&pairs, 5);
    assert_eq!(lines.len(), 2); // 窄宽：每对独占一行（整对不拆）
    assert!(lines[0].spans[0].content.starts_with("status:"));
    assert!(lines[1].spans[0].content.starts_with("kind:"));
}

#[test]
fn kv_lines_wide_pair_degrades_to_single_column() {
    let pairs = vec![
        ("status".to_string(), Span::raw("planned")),
        (
            "body".to_string(),
            Span::raw("a very very long body content that exceeds the width"),
        ),
    ];
    let lines = kv_lines(&pairs, 30);
    // 超宽 body 对 → 独占一行（value 由 wrap 续行，key 前缀保留首行）。
    assert_eq!(lines.len(), 2);
    assert!(lines[0].spans[0].content.starts_with("status"));
    assert!(lines[1].spans[0].content.starts_with("body"));
}
