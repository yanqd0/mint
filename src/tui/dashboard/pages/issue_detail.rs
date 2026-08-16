//! issue 详情页：basic（动态多列键值对）+ tags + test + body + links 多 panel。

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::Color;
use ratatui::text::{Line, Span};

use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::pages::common::{
    body_paragraph, kv_lines, label_style, panel_wrap, status_text_style,
};
use crate::tui::panel::{render_panel, stack};

/// 渲染 issue 详情：basic（键值对动态多列）→ tags → test → body（弹性）→ links。
pub fn draw_detail(frame: &mut Frame, m: &mut DashboardModel, id: i64, area: Rect) {
    let Some(issue) = m.issue(id) else {
        render_panel(
            frame,
            area,
            "issue",
            vec![Line::from(format!("#{id} (deleted)"))],
        );
        return;
    };

    // basic 键值对：有值才显；plan/milestone 只显 ID（#N）。
    let mut kv: Vec<(String, Span<'static>)> = vec![
        (
            "status".into(),
            Span::styled(issue.status.as_str(), status_text_style(issue.status)),
        ),
        ("kind".into(), Span::raw(issue.kind.as_str())),
        ("priority".into(), Span::raw(issue.priority.to_string())),
    ];
    if let Some(p) = &issue.project {
        kv.push(("project".into(), Span::raw(p.clone())));
    }
    if let Some(pid) = issue.plan_id {
        kv.push(("plan".into(), Span::raw(format!("#{pid}"))));
        if let Some(mid) = m
            .plans
            .iter()
            .find(|(c, _)| c.id == pid)
            .and_then(|(c, _)| c.milestone_id)
        {
            kv.push(("milestone".into(), Span::raw(format!("#{mid}"))));
        }
    }
    if let Some(dr) = &issue.dropped_reason {
        kv.push(("dropped".into(), Span::raw(dr.clone())));
    }
    if let Some(sha) = &issue.last_commit_id {
        kv.push(("commit".into(), Span::raw(sha.clone())));
    }
    kv.push((
        "created".into(),
        Span::styled(issue.created_at.clone(), Color::Magenta),
    ));
    kv.push((
        "updated".into(),
        Span::styled(issue.updated_at.clone(), Color::Magenta),
    ));

    let inner_w = area.width.saturating_sub(4);
    let basic_rows = kv_lines(&kv, inner_w);

    // 垂直布局：basic 固定 + 可选 panel（tags/test/body/links），body 弹性。
    let mut constraints: Vec<Constraint> = vec![Constraint::Length(basic_rows.len() as u16 + 2)];
    if !issue.labels.is_empty() {
        constraints.push(Constraint::Length(3));
    }
    if issue.test_cmd.is_some() {
        constraints.push(Constraint::Length(3));
    }
    if issue.body.is_some() {
        constraints.push(Constraint::Min(1));
    }
    if !issue.links.is_empty() {
        constraints.push(Constraint::Length(issue.links.len() as u16 + 2));
    }
    let chunks = stack(area, &constraints);
    let mut ci = 0;

    frame.render_widget(
        panel_wrap(
            &format!("#{} {}", issue.id, issue.title),
            basic_rows,
            chunks[ci].width,
        ),
        chunks[ci],
    );
    ci += 1;

    if !issue.labels.is_empty() {
        // 每个 label 按记录 color 着色（#270，REVERSED chip 效果），空格分隔。
        let mut spans: Vec<Span> = Vec::new();
        for (n, l) in issue.labels.iter().enumerate() {
            if n > 0 {
                spans.push(Span::raw(" "));
            }
            let color = issue.label_colors.get(l).cloned().unwrap_or_default();
            spans.push(Span::styled(l.clone(), label_style(&color)));
        }
        let line = Line::from(spans);
        render_panel(frame, chunks[ci], "tags", vec![line]);
        ci += 1;
    }
    if let Some(tc) = &issue.test_cmd {
        render_panel(frame, chunks[ci], "test", vec![Line::from(tc.clone())]);
        ci += 1;
    }
    if let Some(b) = &issue.body {
        frame.render_widget(body_paragraph(b, "body", chunks[ci].width), chunks[ci]);
        ci += 1;
    }
    if !issue.links.is_empty() {
        let lines: Vec<Line> = issue
            .links
            .iter()
            .map(|l| {
                Line::from(format!(
                    "#{:<4} {:<12} #{:<4} {}",
                    issue.id, l.rel, l.other_id, l.other_title
                ))
            })
            .collect();
        render_panel(frame, chunks[ci], "links", lines);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Link, Status};
    use crate::tui::dashboard::pages::tests_common::{
        buffer_text, mk_issue, model_full, test_backend,
    };

    fn mk_link(other_id: i64, rel: &str) -> Link {
        Link {
            other_id,
            other_title: "other".into(),
            rel: rel.into(),
            created_at: "t".into(),
        }
    }

    /// 渲染 issue 详情，返回逐行文本。
    fn render(m: &mut DashboardModel, id: i64) -> Vec<String> {
        let mut terminal = test_backend(80, 24);
        terminal.draw(|f| draw_detail(f, m, id, f.area())).unwrap();
        buffer_text(terminal.backend().buffer())
    }

    #[test]
    fn basic_panel_shows_kv_and_deleted_fallback() {
        let mut m = model_full(
            vec![mk_issue(7, "hello", Status::Dev, Some(3))],
            vec![],
            vec![],
        );
        let text = render(&mut m, 7).join("\n");
        assert!(text.contains("#7 hello"), "标题: {text}");
        assert!(text.contains("status: dev"), "status 键值: {text}");
        assert!(text.contains("plan: #3"), "plan 显 ID: {text}");
        // 不存在的 issue → deleted 提示
        let text2 = render(&mut m, 99).join("\n");
        assert!(text2.contains("#99 (deleted)"));
    }

    #[test]
    fn tags_test_body_and_links_panels_appear_conditionally() {
        let mut issue = mk_issue(1, "full", Status::Done, None);
        issue.labels = vec!["dev".into(), "urgent".into()];
        issue.test_cmd = Some("cargo test".into());
        issue.body = Some("line1\nline2".into());
        issue.links = vec![mk_link(9, "related")];
        let mut m = model_full(vec![issue], vec![], vec![]);
        let text = render(&mut m, 1).join("\n");
        assert!(text.contains("tags"), "tags panel: {text}");
        assert!(text.contains("dev urgent"), "tags 横排: {text}");
        assert!(text.contains("test"), "test panel: {text}");
        assert!(text.contains("cargo test"), "test 内容: {text}");
        assert!(text.contains("line1"), "body 内容: {text}");
        assert!(text.contains("line2"), "body 多行: {text}");
        assert!(text.contains("links"), "links panel: {text}");
        assert!(text.contains("#9"), "links 目标: {text}");
    }

    #[test]
    fn body_wraps_when_long() {
        let mut issue = mk_issue(1, "long", Status::Open, None);
        issue.body =
            Some("this is a very long body line that should wrap across the terminal width".into());
        let mut m = model_full(vec![issue], vec![], vec![]);
        let text = render(&mut m, 1).join("\n");
        // body 内容换行后仍含开头与结尾片段（长行被 wrap 成多行）。
        assert!(
            text.contains("this is a very long body"),
            "body 开头: {text}"
        );
        assert!(text.contains("terminal width"), "body 结尾: {text}");
    }
}
