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

/// 列表 panel 标题：`{title} · page {cur}/{pages} · size {cur_size}/{total}`。
/// `cur_size` 为当前页实际行数（非分页数）；`total` 为总行数。
/// **不含 `─` 前缀**：Block title 路径由调用方加；render_panel 路径由 `panel_title` 加（避免双重前缀）。
/// 各 list panel（issues/plans/milestones/detail 双段）统一复用（#264）。
pub fn list_title(
    title: &str,
    cur_page: usize,
    total_pages: usize,
    cur_size: usize,
    total: usize,
) -> String {
    format!("{title} · page {cur_page}/{total_pages} · size {cur_size}/{total}")
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

/// 解析 `#hex` 颜色串 → ratatui Color（非法/空 → None）。
fn parse_hex_color(hex: &str) -> Option<Color> {
    let h = hex.trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&h[0..2], 16).ok()?;
    let g = u8::from_str_radix(&h[2..4], 16).ok()?;
    let b = u8::from_str_radix(&h[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}

/// label chip 样式：背景=记录 color，前景按背景亮度自动推定（反差够大）。
/// 亮背景 → 深前景（黑）；暗背景 → 浅前景（白），基于 YIQ 亮度判定。
/// 无 color（空串/非法）→ 默认样式。
pub fn label_style(color: &str) -> Style {
    match parse_hex_color(color) {
        Some(c) => Style::new().bg(c).fg(contrast_fg(c)),
        None => Style::default(),
    }
}

/// 按背景亮度推定前景色：YIQ 亮度 Y=(R*299+G*587+B*114)/1000，
/// Y ≥ 128（亮背景）→ 深前景 Black；否则 → 浅前景 White。
fn contrast_fg(bg: Color) -> Color {
    let Color::Rgb(r, g, b) = bg else {
        return Color::White;
    };
    let y = (r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000;
    if y >= 128 { Color::Black } else { Color::White }
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
            Constraint::Length(n) | Constraint::Max(n) | Constraint::Min(n) => Some(*n),
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
#[path = "common_tests.rs"]
mod tests;
