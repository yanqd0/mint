//! 共享渲染辅助：状态色 / 状态点 / 进度条（各页面复用）。

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::models::{Issue, Status};

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

/// 进度条段样式：亮=未完成、亮闪=在做、暗=完成、红=drop。
fn progress_style(status: Status) -> Style {
    if status == Status::Done {
        Style::new().fg(Color::DarkGray)
    } else {
        status_style(status)
    }
}

/// 进度条：每段 = 一个 issue（open 率可视化）。
pub fn progress_bar(issues: &[&Issue]) -> Line<'static> {
    let spans: Vec<Span> = issues
        .iter()
        .map(|i| Span::styled("█", progress_style(i.status)))
        .collect();
    Line::from(spans)
}

/// 迷你进度条：固定宽度，█ 填充完成比例（milestone 面板每行 plan 用）。
pub fn mini_bar(done: usize, total: usize, width: usize) -> String {
    if total == 0 {
        return "░".repeat(width);
    }
    let filled = done.saturating_mul(width).checked_div(total).unwrap_or(0);
    "█".repeat(filled) + &"░".repeat(width - filled)
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
    fn progress_bar_one_segment_per_issue() {
        let issues = [
            mk_issue(1, Status::Open),
            mk_issue(2, Status::Dev),
            mk_issue(3, Status::Done),
        ];
        let refs: Vec<&Issue> = issues.iter().collect();
        let line = progress_bar(&refs);
        assert_eq!(line.spans.len(), 3);
        assert_eq!(line.spans[0].content, "█");
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
    fn mini_bar_fills_by_ratio() {
        assert_eq!(mini_bar(0, 0, 4), "░".repeat(4));
        assert_eq!(mini_bar(1, 2, 4), "██░░");
        assert_eq!(mini_bar(2, 2, 4), "████");
        assert_eq!(mini_bar(0, 2, 4), "░░░░");
    }
}
