//! 共享渲染辅助：状态色 / 状态点 / 进度条（各页面复用）。

use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Padding, Paragraph, Wrap};

use crate::models::{ContainerStatus, Status};
use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::types::JumpKind;
use crate::tui::panel::panel_title;

/// footer 行：搜索输入态 → `/text█`（光标占位）；搜索提交 → `/text`；否则 help。
pub fn footer_line(m: &DashboardModel, help: &str) -> Line<'static> {
    match &m.search {
        Some(s) if s.active => Line::from(format!("/{}█", s.text)),
        Some(s) => Line::from(format!("/{}", s.text)),
        None => Line::from(help.to_string()),
    }
}

/// 闪烁样式：目标（id+kind）在 `m.flash` 中 → SLOW_BLINK（列表行闪烁标记，变化内容提示）。
pub fn flash_style(m: &DashboardModel, id: i64, kind: JumpKind) -> Option<Style> {
    m.flash
        .iter()
        .find(|f| f.id == id && f.kind == kind)
        .map(|_| Style::new().add_modifier(Modifier::SLOW_BLINK))
}

/// 容器状态基色（milestone/plan 状态点用）。
/// 容器状态基色（对齐 #164 issue 色：open 白 / running 黄 / partial 青 / done 绿 / dropped 红）。
pub fn container_status_color(status: ContainerStatus) -> Color {
    match status {
        ContainerStatus::Open => Color::White,
        ContainerStatus::Running => Color::Yellow,
        ContainerStatus::Partial => Color::Cyan,
        ContainerStatus::Dropped => Color::Red,
        ContainerStatus::Done => Color::Green,
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

/// 状态点：`●` + 颜色（open 白 / planned·dev·test 黄[工作色，点闪] / done 绿 / dropped 红）。
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

/// kind 简写（仅列表显示；requirement→req、problem→bug、task→task）。
pub fn kind_abbrev(kind: crate::models::Kind) -> &'static str {
    match kind {
        crate::models::Kind::Requirement => "req",
        crate::models::Kind::Problem => "bug",
        crate::models::Kind::Task => "task",
    }
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

/// 单行贪心 word-wrap（按空格断行，超宽换新行；词级不拆）。
fn wrap_line(line: &str, width: usize) -> Vec<String> {
    use unicode_width::UnicodeWidthStr;
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut cur_w = 0usize;
    for word in line.split(' ') {
        let ww = word.width();
        let sep = if cur.is_empty() { 0 } else { 1 };
        if !cur.is_empty() && cur_w + sep + ww > width {
            out.push(std::mem::take(&mut cur));
            cur_w = 0;
        }
        if !cur.is_empty() {
            cur.push(' ');
            cur_w += 1;
        }
        cur.push_str(word);
        cur_w += ww;
    }
    out.push(cur);
    out
}

/// body 按 \n split + 贪心 word-wrap，截前 max_lines 行；多余省略（末行加 …）。
/// plan/milestone 详情 body 上限用；issue body 走自然 wrap（见 body_paragraph）。
pub fn body_lines_capped(body: &str, width: usize, max_lines: usize) -> Vec<String> {
    use unicode_width::UnicodeWidthStr;
    let mut out: Vec<String> = Vec::new();
    let mut truncated = false;
    'outer: for seg in body.split('\n') {
        for l in wrap_line(seg, width) {
            if out.len() >= max_lines {
                truncated = true;
                break 'outer;
            }
            out.push(l);
        }
    }
    if truncated {
        out.truncate(max_lines);
        // 末行加 …（若放不下则截断末行）。
        let last = &mut out[max_lines.saturating_sub(1)];
        if last.width() < width {
            last.push('…');
        } else {
            let budget = width.saturating_sub(1);
            let mut s = String::new();
            let mut w = 0usize;
            for c in last.chars() {
                let cw = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
                if w + cw > budget {
                    break;
                }
                s.push(c);
                w += cw;
            }
            s.push('…');
            *last = s;
        }
    }
    out
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
/// 键值对紧凑布局：`key: value | key: value | ...`，**整对**贪心打包进 `Line`（超宽才换行），
/// 一个键值对**不拆两行**（换行以对为单位）。value 可带样式（状态色/时间紫）。
/// 单对超 panel 宽 → 独占一行，由 panel wrap 续行（`key: ` 前缀保留首行）。
/// 返回多行 `Line`，供详情页 basic panel 用（空值由调用方过滤后再传）。
pub fn kv_lines(pairs: &[(String, Span<'static>)], width: u16) -> Vec<Line<'static>> {
    use unicode_width::UnicodeWidthStr;
    if pairs.is_empty() {
        return Vec::new();
    }
    let avail = width as usize;
    let mut lines: Vec<Line> = Vec::new();
    let mut current: Vec<Span> = Vec::new();
    let mut cur_w = 0usize;
    for (k, v) in pairs.iter() {
        let pair_w = k.width() + 2 + v.content.width(); // "key: " + value
        if !current.is_empty() && cur_w + 3 + pair_w > avail {
            lines.push(Line::from(std::mem::take(&mut current)));
            cur_w = 0;
        }
        if !current.is_empty() {
            current.push(Span::raw(" | "));
            cur_w += 3;
        }
        current.push(Span::raw(format!("{k}: ")));
        current.push(v.clone());
        cur_w += pair_w;
    }
    if !current.is_empty() {
        lines.push(Line::from(current));
    }
    lines
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
}
