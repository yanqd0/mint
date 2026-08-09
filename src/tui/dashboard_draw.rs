//! dashboard 渲染：进度条（open 率）+ 状态点 + 面板列表 + footer。

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::models::{Issue, Status};
use crate::tui::dashboard::{DashboardModel, View};

/// 状态点：`●` + 颜色（黄=待做、绿闪=开发、绿=在做、白=完成、红=drop）。
pub fn status_dot(status: Status) -> (char, Style) {
    let style = match status {
        Status::Open => Style::new().fg(Color::Yellow),
        Status::Planned => Style::new()
            .fg(Color::Yellow)
            .add_modifier(Modifier::SLOW_BLINK),
        Status::Dev => Style::new()
            .fg(Color::Green)
            .add_modifier(Modifier::SLOW_BLINK),
        Status::Test => Style::new().fg(Color::Green),
        Status::Done => Style::new().fg(Color::White),
        Status::Dropped => Style::new().fg(Color::Red),
    };
    ('●', style)
}

/// 进度条段样式：亮=未完成、亮闪=在做、暗=完成、暗红=drop。
fn progress_style(status: Status) -> Style {
    match status {
        Status::Open => Style::new().fg(Color::Yellow),
        Status::Planned => Style::new()
            .fg(Color::Yellow)
            .add_modifier(Modifier::SLOW_BLINK),
        Status::Dev | Status::Test => Style::new()
            .fg(Color::Green)
            .add_modifier(Modifier::SLOW_BLINK),
        Status::Done => Style::new().fg(Color::DarkGray),
        Status::Dropped => Style::new().fg(Color::Red),
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

/// 面板标题。
fn panel_title(m: &DashboardModel) -> String {
    match m.view {
        View::Issue => "mint · issues".to_string(),
        View::Plan { plan_id } => format!("mint · plan #{plan_id}"),
    }
}

/// 渲染 issue 详情（Enter 展开）。
fn draw_detail(frame: &mut Frame, m: &DashboardModel, id: i64) {
    let mut lines: Vec<Line> = Vec::new();
    if let Some(i) = m.issue(id) {
        lines.push(Line::from(format!("#{} {}", i.id, i.title)));
        lines.push(Line::from(format!("  status:   {}", i.status.as_str())));
        lines.push(Line::from(format!("  kind:     {}", i.kind.as_str())));
        lines.push(Line::from(format!("  priority: {}", i.priority)));
        if let Some(p) = &i.project {
            lines.push(Line::from(format!("  project:  {p}")));
        }
        if let Some(pid) = i.plan_id {
            lines.push(Line::from(format!("  plan:     #{pid}")));
        }
        if !i.labels.is_empty() {
            lines.push(Line::from(format!("  labels:   {}", i.labels.join(", "))));
        }
        if let Some(b) = &i.body {
            lines.push(Line::from(format!("  body:     {b}")));
        }
        if let Some(tc) = &i.test_cmd {
            lines.push(Line::from(format!("  test:     {tc}")));
        }
        if let Some(dr) = &i.dropped_reason {
            lines.push(Line::from(format!("  dropped:  {dr}")));
        }
        if let Some(sha) = &i.last_commit_id {
            lines.push(Line::from(format!("  commit:   {sha}")));
        }
        if !i.links.is_empty() {
            lines.push(Line::from(format!("  links:    {}", i.links.len())));
            for l in &i.links {
                lines.push(Line::from(format!(
                    "    #{:<4} {:<12} #{:<4} {}",
                    i.id, l.rel, l.other_id, l.other_title
                )));
            }
        }
        lines.push(Line::from(format!("  created:  {}", i.created_at)));
        lines.push(Line::from(format!("  updated:  {}", i.updated_at)));
    } else {
        lines.push(Line::from(format!("#{id} (deleted)")));
    }
    frame.render_widget(
        Paragraph::new(lines).block(Block::bordered().title("detail")),
        frame.area(),
    );
}

/// 渲染 dashboard：详情视图或面板（进度条 + 状态点列表）+ footer。
pub fn draw_dashboard(frame: &mut Frame, m: &DashboardModel) {
    if let Some(id) = m.detail {
        return draw_detail(frame, m, id);
    }
    let all = m.visible_issues();
    let page = m.page_issues();
    let total = all
        .iter()
        .filter(|i| !matches!(i.status, Status::Dropped))
        .count();
    let done = all
        .iter()
        .filter(|i| matches!(i.status, Status::Done))
        .count();
    let progress_rate = done
        .checked_mul(100)
        .and_then(|d| d.checked_div(total))
        .unwrap_or(0);

    let mut lines: Vec<Line> = Vec::new();
    lines.push(progress_bar(&all));
    lines.push(Line::from(format!("  progress: {progress_rate}%")));
    for (idx, i) in page.iter().enumerate() {
        let (dot, dot_style) = status_dot(i.status);
        let selected = idx == m.selected;
        let text_style = if selected {
            Style::new().add_modifier(Modifier::REVERSED)
        } else {
            Style::new()
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{dot} "), dot_style),
            Span::styled(
                format!("#{} {} {}", i.id, i.status.as_str(), i.title),
                text_style,
            ),
        ]));
    }

    let footer = format!(
        "j/k ↑↓ row · h/l PgUp/PgDn page · Tab plan · Enter detail · q quit · Page {}/{}",
        m.page + 1,
        m.pages()
    );
    let chunks = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(frame.area());
    frame.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(panel_title(m))),
        chunks[0],
    );
    frame.render_widget(Paragraph::new(Line::from(footer)), chunks[1]);
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::models::Kind;
    use crate::tui::dashboard::DashboardModel;
    use crate::tui::dashboard_diff::DashboardSnapshot;

    fn mk_issue(id: i64, title: &str, status: Status, plan_id: Option<i64>) -> Issue {
        Issue {
            id,
            title: title.into(),
            body: None,
            kind: Kind::Problem,
            status,
            priority: 3,
            project_id: 1,
            project: Some("mint".into()),
            test_cmd: None,
            dropped_reason: None,
            last_commit_id: None,
            plan_id,
            hit_count: 0,
            labels: vec![],
            links: vec![],
            created_at: "t".into(),
            updated_at: "t".into(),
        }
    }

    fn model_with(issues: Vec<Issue>) -> DashboardModel {
        let mut m = DashboardModel::new();
        m.init(DashboardSnapshot {
            issues,
            plans: vec![],
        });
        m
    }

    #[test]
    fn progress_bar_one_segment_per_issue() {
        let issues = [
            mk_issue(1, "a", Status::Open, None),
            mk_issue(2, "b", Status::Dev, None),
            mk_issue(3, "c", Status::Done, None),
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
    fn draw_issue_panel_shows_title_rate_and_dot() {
        let m = model_with(vec![
            mk_issue(1, "open one", Status::Open, None),
            mk_issue(2, "done one", Status::Done, None),
        ]);
        let backend = TestBackend::new(60, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw_dashboard(f, &m)).unwrap();
        let text = buffer_text(terminal.backend().buffer());
        let joined = text.join("\n");
        assert!(joined.contains("mint · issues"), "标题: {joined}");
        assert!(joined.contains("progress: 50%"), "rate: {joined}");
        assert!(joined.contains("#1 open"), "issue 行: {joined}");
        assert!(joined.contains("●"), "状态点: {joined}");
    }

    #[test]
    fn draw_plan_panel_filters_issues() {
        let mut m = model_with(vec![
            mk_issue(1, "in plan", Status::Dev, Some(7)),
            mk_issue(2, "outside", Status::Open, None),
        ]);
        m.view = View::Plan { plan_id: 7 };
        m.selected = 0;
        let backend = TestBackend::new(60, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw_dashboard(f, &m)).unwrap();
        let text = buffer_text(terminal.backend().buffer()).join("\n");
        assert!(text.contains("plan #7"), "标题: {text}");
        assert!(text.contains("in plan"), "应含 plan issue: {text}");
        assert!(!text.contains("outside"), "不应含外部 issue: {text}");
    }

    #[test]
    fn draw_detail_shows_issue_fields() {
        let mut m = model_with(vec![mk_issue(1, "hello", Status::Dev, Some(7))]);
        m.detail = Some(1);
        let backend = TestBackend::new(60, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw_dashboard(f, &m)).unwrap();
        let text = buffer_text(terminal.backend().buffer()).join("\n");
        assert!(text.contains("#1 hello"), "标题: {text}");
        assert!(text.contains("status:"), "字段: {text}");
        assert!(text.contains("plan:"), "plan 字段: {text}");
    }
}
