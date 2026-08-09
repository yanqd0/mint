//! dashboard 渲染：进度条（open 率）+ 状态点 + 面板列表 + footer。

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::models::{Issue, Status};
use crate::tui::dashboard::{DashboardModel, View};

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
fn mini_bar(done: usize, total: usize, width: usize) -> String {
    if total == 0 {
        return "░".repeat(width);
    }
    let filled = done.saturating_mul(width).checked_div(total).unwrap_or(0);
    "█".repeat(filled) + &"░".repeat(width - filled)
}

/// 面板标题。
fn panel_title(m: &DashboardModel) -> String {
    match m.view {
        View::Issue => "mint · issues".to_string(),
        View::Plan { plan_id } => format!("mint · plan #{plan_id}"),
        View::Milestone { milestone_id } => format!("mint · milestone #{milestone_id}"),
    }
}

/// 渲染 issue 详情（Enter 展开）。
fn draw_detail(frame: &mut Frame, m: &DashboardModel, id: i64) {
    let mut lines: Vec<Line> = Vec::new();
    if let Some(i) = m.issue(id) {
        lines.push(Line::from(format!("#{} {}", i.id, i.title)));
        lines.push(Line::from(vec![
            Span::raw("  status:   "),
            Span::styled(i.status.as_str(), status_text_style(i.status)),
        ]));
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
        if let Some(b) = &i.body {
            lines.push(Line::from(format!("  body:     {b}")));
        }
    } else {
        lines.push(Line::from(format!("#{id} (deleted)")));
    }
    frame.render_widget(
        Paragraph::new(lines).block(Block::bordered().title("detail")),
        frame.area(),
    );
}

/// 渲染 milestone 面板：自身标题 + 其下 plan 行列表（每行迷你进度条 + done/total）。
fn draw_milestone(frame: &mut Frame, m: &DashboardModel) {
    let View::Milestone { milestone_id } = m.view else {
        return;
    };
    let title = m
        .milestones
        .iter()
        .find(|(c, _)| c.id == milestone_id)
        .map(|(c, _)| match &c.version {
            Some(v) => format!("{} ({v})", c.title),
            None => c.title.clone(),
        })
        .unwrap_or_else(|| format!("#{milestone_id}"));

    let mut lines: Vec<Line> = Vec::new();
    for (idx, (plan, _)) in m.page_plans().iter().enumerate() {
        let (done, total) = m.plan_progress(plan.id);
        let selected = idx == m.selected;
        let style = if selected {
            Style::new().add_modifier(Modifier::REVERSED)
        } else {
            Style::new()
        };
        let bar = mini_bar(done, total, 20);
        lines.push(Line::from(vec![
            Span::styled(format!("#{:<3}", plan.id), style),
            Span::styled(format!("[{bar}]"), style),
            Span::styled(format!(" {done}/{total}  {}", plan.title), style),
        ]));
    }
    if lines.is_empty() {
        lines.push(Line::from("(no plans in this milestone)"));
    }

    let footer = format!(
        "j/k ↑↓ plan · h/l PgUp/PgDn page · Enter plan · Esc back · q quit · Page {}/{}",
        m.page + 1,
        m.plan_pages()
    );
    let chunks = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(frame.area());
    frame.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(format!("mint · milestone {title}"))),
        chunks[0],
    );
    frame.render_widget(Paragraph::new(Line::from(footer)), chunks[1]);
}

/// 渲染 dashboard：详情视图或面板（进度条 + 状态点列表）+ footer。
pub fn draw_dashboard(frame: &mut Frame, m: &DashboardModel) {
    if let Some(id) = m.detail {
        return draw_detail(frame, m, id);
    }
    if matches!(m.view, View::Milestone { .. }) {
        return draw_milestone(frame, m);
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
            status_text_style(i.status).add_modifier(Modifier::REVERSED)
        } else {
            status_text_style(i.status)
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
    use crate::models::{Container, ContainerStatus, Kind};
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
            milestones: vec![],
        });
        m
    }

    fn mk_container(
        id: i64,
        title: &str,
        version: Option<&str>,
        milestone_id: Option<i64>,
    ) -> Container {
        Container {
            id,
            title: title.into(),
            version: version.map(String::from),
            body: None,
            milestone_id,
            status: ContainerStatus::Running,
            created_at: "t".into(),
            updated_at: "t".into(),
        }
    }

    fn model_full(
        issues: Vec<Issue>,
        plans: Vec<(Container, i64)>,
        milestones: Vec<(Container, i64)>,
    ) -> DashboardModel {
        let mut m = DashboardModel::new();
        m.init(DashboardSnapshot {
            issues,
            plans,
            milestones,
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

    #[test]
    fn mini_bar_fills_by_ratio() {
        assert_eq!(mini_bar(0, 0, 4), "░".repeat(4));
        assert_eq!(mini_bar(1, 2, 4), "██░░");
        assert_eq!(mini_bar(2, 2, 4), "████");
        assert_eq!(mini_bar(0, 2, 4), "░░░░");
    }

    #[test]
    fn draw_milestone_panel_shows_plan_rows_with_progress() {
        let mut m = model_full(
            vec![
                mk_issue(1, "done one", Status::Done, Some(7)),
                mk_issue(2, "open one", Status::Open, Some(7)),
            ],
            vec![(mk_container(7, "tui plan", None, Some(4)), 0)],
            vec![(mk_container(4, "TUI", Some("0.4.0"), None), 0)],
        );
        m.view = View::Milestone { milestone_id: 4 };
        m.selected = 0;
        let backend = TestBackend::new(70, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw_dashboard(f, &m)).unwrap();
        let text = buffer_text(terminal.backend().buffer()).join("\n");
        assert!(text.contains("milestone TUI (0.4.0)"), "标题: {text}");
        assert!(text.contains("tui plan"), "plan 标题: {text}");
        assert!(text.contains("1/2"), "进度: {text}");
    }
}
