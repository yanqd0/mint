//! 共享渲染辅助：状态色 / 状态点 / 进度条（各页面复用）。

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Padding, Paragraph, Wrap};

use crate::models::{ContainerStatus, Issue, Status};
use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::types::JumpKind;

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

/// 状态基色（点/文字共用，TUI 统一配色）。
fn status_color(status: Status) -> Color {
    match status {
        Status::Open | Status::Planned => Color::Yellow,
        Status::Dev | Status::Test => Color::Green,
        Status::Done => Color::White,
        Status::Dropped => Color::Red,
    }
}

/// 状态是否闪烁（Planned 已排期待做、Dev 开发中）。
fn status_blinks(status: Status) -> bool {
    matches!(status, Status::Planned | Status::Dev)
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

/// 进度条段样式：open 暗黄 / planned 亮黄 / dev 暗绿 / test 亮绿 / done 白 / dropped 亮红（计入完成）。
fn progress_style(status: Status) -> Style {
    match status {
        Status::Open => Style::new().fg(Color::Yellow).add_modifier(Modifier::DIM),
        Status::Planned => Style::new().fg(Color::Yellow),
        Status::Dev => Style::new().fg(Color::Green).add_modifier(Modifier::DIM),
        Status::Test => Style::new().fg(Color::Green),
        Status::Done => Style::new().fg(Color::White),
        Status::Dropped => Style::new().fg(Color::Red),
    }
}

/// 进度条：定长按占比分段的彩色条——每 issue 一段（颜色按状态），
/// 段宽 = `width / N`（末段取余补足），issue 数变化占比自动调整。
pub fn progress_bar(issues: &[&Issue], width: usize) -> Line<'static> {
    let n = issues.len();
    if n == 0 || width == 0 {
        return Line::from(String::new());
    }
    let seg = width / n;
    let mut spans: Vec<Span> = Vec::with_capacity(n);
    for (i, issue) in issues.iter().enumerate() {
        let w = if i == n - 1 {
            width - seg * (n - 1)
        } else {
            seg
        };
        if w > 0 {
            spans.push(Span::styled("█".repeat(w), progress_style(issue.status)));
        }
    }
    Line::from(spans)
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

/// 按显示宽度截断字符串（超出加 …，总宽 ≤ max；kanban 列标题等用）。
pub fn truncate(s: &str, max: usize) -> String {
    use unicode_width::UnicodeWidthStr;
    if s.width() <= max {
        return s.to_string();
    }
    let budget = max.saturating_sub(1); // 预留 … 的宽度
    let mut out = String::new();
    let mut w = 0;
    for c in s.chars() {
        let cw = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
        if w + cw > budget {
            break;
        }
        out.push(c);
        w += cw;
    }
    out.push('…');
    out
}

/// body 段落：wrap 多行 + 圆角 border + 标题（issue/plan/milestone 详情 body panel 共用）。
/// 内容左右 1 格 padding（全局 margin 配置）。
pub fn body_paragraph(body: &str, title: &str) -> Paragraph<'static> {
    let lines: Vec<Line> = body
        .split('\n')
        .map(|l| Line::from(l.to_string()))
        .collect();
    Paragraph::new(lines).wrap(Wrap { trim: true }).block(
        Block::bordered()
            .border_type(BorderType::Rounded)
            .title(title.to_string())
            .padding(Padding::horizontal(1)),
    )
}

/// 键值对多列布局：按可用宽度估算列数，逐列对齐（列宽 = 该列最宽 + 2 padding）。
/// 返回多行 `Line`，供详情页 basic panel 用（空值由调用方过滤后再传）。
pub fn kv_lines(pairs: &[(String, String)], width: u16) -> Vec<Line<'static>> {
    use unicode_width::UnicodeWidthStr;
    if pairs.is_empty() {
        return Vec::new();
    }
    let cells: Vec<(String, usize)> = pairs
        .iter()
        .map(|(k, v)| {
            let s = format!("{k}: {v}");
            let w = s.width();
            (s, w)
        })
        .collect();
    let total_w: usize = cells.iter().map(|(_, w)| w).sum();
    let avg = total_w / cells.len();
    let cols = (width as usize / avg.max(1)).clamp(1, cells.len());
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
    use crate::models::Kind;

    fn mk_issue(id: i64, status: Status) -> Issue {
        Issue {
            id,
            title: "t".into(),
            body: None,
            kind: Kind::Problem,
            status,
            priority: 3,
            project_id: 1,
            project: Some("mint".into()),
            test_cmd: None,
            dropped_reason: None,
            last_commit_id: None,
            plan_id: None,
            hit_count: 0,
            labels: vec![],
            links: vec![],
            created_at: "t".into(),
            updated_at: "t".into(),
        }
    }

    #[test]
    fn progress_bar_shares_width_and_colors_by_status() {
        use ratatui::style::Color as C;
        let issues = [
            mk_issue(1, Status::Open),
            mk_issue(2, Status::Done),
            mk_issue(3, Status::Dropped),
        ];
        let refs: Vec<&Issue> = issues.iter().collect();
        let line = progress_bar(&refs, 9);
        // 3 issue / 宽 9 → 每段 3 格，总宽 9（末段取余）。
        let total: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
        assert_eq!(total, 9);
        assert_eq!(line.spans.len(), 3);
        // open 暗黄（DIM）、dropped 亮红。
        assert_eq!(line.spans[0].style.fg, Some(C::Yellow));
        assert!(line.spans[0].style.add_modifier.contains(Modifier::DIM));
        assert_eq!(line.spans[2].style.fg, Some(C::Red));
    }

    #[test]
    fn status_dot_colors() {
        use ratatui::style::Color as C;
        assert_eq!(status_dot(Status::Open).0, '●');
        assert_eq!(status_dot(Status::Open).1.fg, Some(C::Yellow));
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
        assert_eq!(status_dot(Status::Test).1.fg, Some(C::Green));
        assert_eq!(status_dot(Status::Done).1.fg, Some(C::White));
        assert_eq!(status_dot(Status::Dropped).1.fg, Some(C::Red));
    }

    #[test]
    fn status_text_style_same_color_no_blink() {
        use ratatui::style::Color as C;
        assert_eq!(status_text_style(Status::Dev).fg, Some(C::Green));
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
        assert_eq!(status_text_style(Status::Done).fg, Some(C::White));
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
        assert!(
            text.starts_with("status: planned  kind: problem"),
            "多列应对齐: {text}"
        );
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
}
