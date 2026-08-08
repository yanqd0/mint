//! 人类可读输出（--json 由 serde 直接序列化，不经此处）。

use crate::models::{Container, Issue, IssueSummary};

/// 渲染 issue 列表（人类可读，每行一个）。
pub fn format_list(issues: &[Issue]) -> String {
    let mut out = String::new();
    for i in issues {
        let label_str = if i.labels.is_empty() {
            String::new()
        } else {
            format!(" [{}]", i.labels.join(","))
        };
        out.push_str(&format!(
            "#{:<4} {:<10} {:<14} {}{}\n",
            i.id,
            i.kind.as_str(),
            i.status.as_str(),
            i.title,
            label_str
        ));
    }
    out
}

/// 渲染单个 issue 详情（人类可读，多行缩进）。
pub fn format_issue(i: &Issue) -> String {
    let mut out = String::new();
    out.push_str(&format!("#{} {}\n", i.id, i.title));
    out.push_str(&format!("  status:  {}\n", i.status.as_str()));
    out.push_str(&format!("  kind:    {}\n", i.kind.as_str()));
    out.push_str(&format!(
        "  project: {}\n",
        i.project.as_deref().unwrap_or("?")
    ));
    if let Some(b) = &i.body {
        out.push_str(&format!("  body:    {b}\n"));
    }
    if i.hit_count > 0 {
        out.push_str(&format!("  hit:     {}\n", i.hit_count));
    }
    if let Some(tc) = &i.test_cmd {
        out.push_str(&format!("  test:    {tc}\n"));
    }
    if let Some(dr) = &i.dropped_reason {
        out.push_str(&format!("  dropped: {dr}\n"));
    }
    if let Some(sha) = &i.last_commit_id {
        out.push_str(&format!("  commit:  {sha}\n"));
    }
    if !i.labels.is_empty() {
        out.push_str(&format!("  labels:  {}\n", i.labels.join(", ")));
    }
    if !i.links.is_empty() {
        out.push_str(&format!("  links:    {}\n", i.links.len()));
        for l in &i.links {
            out.push_str(&format!(
                "    #{:<4} {:<12} #{:<4} {}\n",
                i.id, l.rel, l.other_id, l.other_title
            ));
        }
    }
    out.push_str(&format!("  created: {}\n", i.created_at));
    out.push_str(&format!("  updated: {}\n", i.updated_at));
    out
}

/// 渲染容器列表（人类可读，每行一个，含子项计数）。
pub fn format_container_list(items: &[(Container, i64)]) -> String {
    let mut out = String::new();
    for (c, count) in items {
        let version = c
            .version
            .as_deref()
            .map(|v| format!(" ({v})"))
            .unwrap_or_default();
        out.push_str(&format!(
            "#{:<4} {:<10} {:<8} {}{} issue{}\n",
            c.id,
            c.status.as_str(),
            count,
            c.title,
            version,
            if *count == 1 { "" } else { "s" }
        ));
    }
    out
}

/// 渲染容器详情（人类可读，多行缩进）+ 其下 issue 列表。
pub fn format_container_show(c: &Container, issues: &[IssueSummary]) -> String {
    let mut out = String::new();
    out.push_str(&format!("#{} {}\n", c.id, c.title));
    out.push_str(&format!("  status:  {}\n", c.status.as_str()));
    if let Some(v) = &c.version {
        out.push_str(&format!("  version: {v}\n"));
    }
    if let Some(rid) = c.roadmap_id {
        out.push_str(&format!("  roadmap: #{rid}\n"));
    }
    if let Some(b) = &c.body {
        out.push_str(&format!("  body:    {b}\n"));
    }
    out.push_str(&format!("  issues:  {}\n", issues.len()));
    for i in issues {
        out.push_str(&format!(
            "    #{:<4} {:<10} {:<14} {}\n",
            i.id,
            i.kind.as_str(),
            i.status.as_str(),
            i.title
        ));
    }
    out.push_str(&format!("  created: {}\n", c.created_at));
    out.push_str(&format!("  updated: {}\n", c.updated_at));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ContainerStatus, Kind, Link, Status};
    use rstest::rstest;

    #[allow(clippy::too_many_arguments)] // 测试构造 helper：字段多但固定，builder 收益低
    fn mk_issue(
        id: i64,
        title: &str,
        kind: Kind,
        status: Status,
        body: Option<&str>,
        test_cmd: Option<&str>,
        dropped: Option<&str>,
        commit: Option<&str>,
        labels: &[&str],
        links: Vec<Link>,
    ) -> Issue {
        Issue {
            id,
            title: title.into(),
            body: body.map(Into::into),
            kind,
            status,
            project_id: 1,
            project: Some("mint".into()),
            test_cmd: test_cmd.map(Into::into),
            dropped_reason: dropped.map(Into::into),
            last_commit_id: commit.map(Into::into),
            plan_id: None,
            hit_count: 0,
            labels: labels.iter().map(|s| s.to_string()).collect(),
            links,
            created_at: "2026-01-01 00:00:00".into(),
            updated_at: "2026-01-01 00:00:00".into(),
        }
    }

    fn mk_container(
        id: i64,
        title: &str,
        version: Option<&str>,
        body: Option<&str>,
        roadmap_id: Option<i64>,
        status: ContainerStatus,
    ) -> Container {
        Container {
            id,
            title: title.into(),
            version: version.map(Into::into),
            body: body.map(Into::into),
            roadmap_id,
            status,
            created_at: "2026-01-01 00:00:00".into(),
            updated_at: "2026-01-01 00:00:00".into(),
        }
    }

    fn mk_summary(id: i64, title: &str, kind: Kind, status: Status) -> IssueSummary {
        IssueSummary {
            id,
            title: title.into(),
            kind,
            status,
            project: Some("mint".into()),
        }
    }

    fn mk_link(other_id: i64, other_title: &str, rel: &str) -> Link {
        Link {
            other_id,
            other_title: other_title.into(),
            rel: rel.into(),
            created_at: "2026-01-01 00:00:00".into(),
        }
    }

    /// format_list：行数 + 关键内容。
    #[rstest]
    #[case::empty(vec![], 0, "")]
    #[case::one(vec![mk_issue(1, "hello", Kind::Problem, Status::Open, None, None, None, None, &[], vec![])], 1, "hello")]
    #[case::many(
        vec![
            mk_issue(1, "first", Kind::Problem, Status::Open, None, None, None, None, &[], vec![]),
            mk_issue(2, "second", Kind::Requirement, Status::Done, None, None, None, None, &[], vec![]),
        ],
        2,
        "second",
    )]
    fn format_list_content(#[case] issues: Vec<Issue>, #[case] lines: usize, #[case] needle: &str) {
        let out = format_list(&issues);
        assert_eq!(out.lines().count(), lines);
        assert!(out.contains(needle), "缺 {needle}: {out}");
    }

    /// format_list：labels 拼接（有/无/多个）。
    #[rstest]
    #[case(&[], "")]
    #[case(&["dev"], "[dev]")]
    #[case(&["dev", "urgent"], "[dev,urgent]")]
    fn format_list_labels(#[case] labels: &[&str], #[case] expected: &str) {
        let out = format_list(&[mk_issue(
            1,
            "t",
            Kind::Problem,
            Status::Open,
            None,
            None,
            None,
            None,
            labels,
            vec![],
        )]);
        if expected.is_empty() {
            assert!(!out.contains('['), "无 labels 不应有括号: {out}");
        } else {
            assert!(out.contains(expected), "缺 {expected}: {out}");
        }
    }

    /// format_issue：可选字段的出现/缺失（body/test_cmd/dropped/commit/labels/links）。
    #[rstest]
    #[case::minimal(
        mk_issue(1, "t", Kind::Problem, Status::Open, None, None, None, None, &[], vec![]),
        &["#1 t", "status:  open", "project: mint"],
        &["body:", "test:", "dropped:", "commit:", "labels:", "links:"],
    )]
    #[case::full(
        mk_issue(
            1,
            "t",
            Kind::Problem,
            Status::Done,
            Some("body text"),
            Some("cargo test"),
            Some("why"),
            Some("abc123"),
            &["dev"],
            vec![mk_link(2, "other", "related")],
        ),
        &["body:", "test:    cargo test", "dropped: why", "commit:  abc123", "labels:  dev", "links:"],
        &[],
    )]
    #[case::partial(
        mk_issue(1, "t", Kind::Problem, Status::Dev, None, Some("cmd"), None, None, &[], vec![]),
        &["test:    cmd"],
        &["body:", "dropped:", "commit:", "labels:", "links:"],
    )]
    fn format_issue_fields(#[case] i: Issue, #[case] present: &[&str], #[case] absent: &[&str]) {
        let out = format_issue(&i);
        for p in present {
            assert!(out.contains(p), "应含 {p}:\n{out}");
        }
        for a in absent {
            assert!(!out.contains(a), "不应含 {a}:\n{out}");
        }
    }

    /// format_issue：hit_count 仅在 >0 时显示（去重命中计数）。
    #[rstest]
    #[case(0, false)]
    #[case(3, true)]
    fn format_issue_hit_count(#[case] n: i64, #[case] show: bool) {
        let mut i = mk_issue(
            1,
            "t",
            Kind::Problem,
            Status::Open,
            None,
            None,
            None,
            None,
            &[],
            vec![],
        );
        i.hit_count = n;
        let out = format_issue(&i);
        assert_eq!(out.contains("hit:"), show, "hit_count={n} 显示: {out}");
    }

    /// format_container_list：行数、version 括号出现、count 显示（空列表无 count）。
    #[rstest]
    #[case::empty(vec![], 0, false)]
    #[case::with_version(vec![(mk_container(1, "r", Some("0.3.0"), None, None, ContainerStatus::Open), 1)], 1, true)]
    #[case::no_version(vec![(mk_container(1, "p", None, None, Some(9), ContainerStatus::Done), 3)], 1, false)]
    fn format_container_list_basic(
        #[case] items: Vec<(Container, i64)>,
        #[case] lines: usize,
        #[case] has_version: bool,
    ) {
        let out = format_container_list(&items);
        assert_eq!(out.lines().count(), lines);
        assert_eq!(out.contains('('), has_version);
        if !items.is_empty() {
            assert!(out.contains(&items[0].1.to_string()), "缺 count: {out}");
        }
    }

    /// format_container_list：单复数切换（issue / issues 行尾）。
    #[rstest]
    #[case(1, "issue\n")]
    #[case(2, "issues\n")]
    fn format_container_list_plural(#[case] n: i64, #[case] expected: &str) {
        let out = format_container_list(&[(
            mk_container(1, "r", None, None, None, ContainerStatus::Open),
            n,
        )]);
        assert!(out.ends_with(expected), "行尾不匹配 {expected:?}: {out}");
    }

    /// format_container_show：roadmap/plan 字段与 issue 计数。
    #[rstest]
    #[case::roadmap(
        mk_container(1, "r", Some("0.3.0"), Some("body"), None, ContainerStatus::Open),
        vec![],
        "version: 0.3.0",
        "issues:  0",
    )]
    #[case::plan(
        mk_container(2, "p", None, None, Some(9), ContainerStatus::Done),
        vec![mk_summary(1, "s", Kind::Problem, Status::Done)],
        "roadmap: #9",
        "issues:  1",
    )]
    fn format_container_show_fields(
        #[case] c: Container,
        #[case] issues: Vec<IssueSummary>,
        #[case] present: &str,
        #[case] count_line: &str,
    ) {
        let out = format_container_show(&c, &issues);
        assert!(out.contains(present), "应含 {present}:\n{out}");
        assert!(out.contains(count_line), "应含 {count_line}:\n{out}");
    }
}
