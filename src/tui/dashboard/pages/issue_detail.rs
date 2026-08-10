//! issue 详情页：basic（动态多列键值对）+ tags + test + body + links 多 panel。

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::text::Line;

use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::pages::common::{body_paragraph, kv_lines, panel_wrap};
use crate::tui::panel::{render_panel, stack};

/// 渲染 issue 详情：basic（键值对动态多列）→ tags → test → body（弹性）→ links。
pub fn draw_detail(frame: &mut Frame, m: &DashboardModel, id: i64, area: Rect) {
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
    let mut kv: Vec<(String, String)> = vec![
        ("status".into(), issue.status.as_str().to_string()),
        ("kind".into(), issue.kind.as_str().to_string()),
        ("priority".into(), issue.priority.to_string()),
    ];
    if let Some(p) = &issue.project {
        kv.push(("project".into(), p.clone()));
    }
    if let Some(pid) = issue.plan_id {
        kv.push(("plan".into(), format!("#{pid}")));
        if let Some(mid) = m
            .plans
            .iter()
            .find(|(c, _)| c.id == pid)
            .and_then(|(c, _)| c.milestone_id)
        {
            kv.push(("milestone".into(), format!("#{mid}")));
        }
    }
    if let Some(dr) = &issue.dropped_reason {
        kv.push(("dropped".into(), dr.clone()));
    }
    if let Some(sha) = &issue.last_commit_id {
        kv.push(("commit".into(), sha.clone()));
    }
    kv.push(("created".into(), issue.created_at.clone()));
    kv.push(("updated".into(), issue.updated_at.clone()));

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
        panel_wrap(&format!("#{} {}", issue.id, issue.title), basic_rows),
        chunks[ci],
    );
    ci += 1;

    if !issue.labels.is_empty() {
        render_panel(
            frame,
            chunks[ci],
            "tags",
            vec![Line::from(issue.labels.join(" "))],
        );
        ci += 1;
    }
    if let Some(tc) = &issue.test_cmd {
        render_panel(frame, chunks[ci], "test", vec![Line::from(tc.clone())]);
        ci += 1;
    }
    if let Some(b) = &issue.body {
        frame.render_widget(body_paragraph(b, "body"), chunks[ci]);
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
    fn render(m: &DashboardModel, id: i64) -> Vec<String> {
        let mut terminal = test_backend(80, 24);
        terminal.draw(|f| draw_detail(f, m, id, f.area())).unwrap();
        buffer_text(terminal.backend().buffer())
    }

    #[test]
    fn basic_panel_shows_kv_and_deleted_fallback() {
        let m = model_full(
            vec![mk_issue(7, "hello", Status::Dev, Some(3))],
            vec![],
            vec![],
        );
        let text = render(&m, 7).join("\n");
        assert!(text.contains("#7 hello"), "标题: {text}");
        assert!(text.contains("status  : dev"), "status 键值: {text}");
        assert!(text.contains("plan    : #3"), "plan 显 ID: {text}");
        // 不存在的 issue → deleted 提示
        let text2 = render(&m, 99).join("\n");
        assert!(text2.contains("#99 (deleted)"));
    }

    #[test]
    fn tags_test_body_and_links_panels_appear_conditionally() {
        let mut issue = mk_issue(1, "full", Status::Done, None);
        issue.labels = vec!["dev".into(), "urgent".into()];
        issue.test_cmd = Some("cargo test".into());
        issue.body = Some("line1\nline2".into());
        issue.links = vec![mk_link(9, "related")];
        let m = model_full(vec![issue], vec![], vec![]);
        let text = render(&m, 1).join("\n");
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
        let m = model_full(vec![issue], vec![], vec![]);
        let text = render(&m, 1).join("\n");
        // body 内容换行后仍含开头与结尾片段（长行被 wrap 成多行）。
        assert!(
            text.contains("this is a very long body"),
            "body 开头: {text}"
        );
        assert!(text.contains("terminal width"), "body 结尾: {text}");
    }
}
