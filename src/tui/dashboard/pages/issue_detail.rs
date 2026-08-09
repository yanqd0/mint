//! issue 详情页：基本信息 + 字段列表（Enter 展开，Esc 收起）。

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::pages::common::status_text_style;

/// 渲染 issue 详情（Enter 展开）。
pub fn draw_detail(frame: &mut Frame, m: &DashboardModel, id: i64, area: Rect) {
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
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Status;
    use crate::tui::dashboard::pages::tests_common::{
        buffer_text, mk_issue, model_with, test_backend,
    };

    #[test]
    fn draw_detail_shows_issue_fields() {
        let mut m = model_with(vec![mk_issue(1, "hello", Status::Dev, Some(7))]);
        m.view = crate::tui::dashboard::types::View::IssueDetail { id: 1 };
        let mut terminal = test_backend(60, 12);
        terminal.draw(|f| draw_detail(f, &m, 1, f.area())).unwrap();
        let text = buffer_text(terminal.backend().buffer()).join("\n");
        assert!(text.contains("#1 hello"), "标题: {text}");
        assert!(text.contains("status:"), "字段: {text}");
        assert!(text.contains("plan:"), "plan 字段: {text}");
    }
}
