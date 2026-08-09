//! 人类可读输出（--json 由 serde 直接序列化，不经此处）。

use crate::models::{Container, Issue, IssueSummary};

/// 渲染单个 issue 详情（人类可读，多行缩进）。
pub fn format_issue(i: &Issue) -> String {
    let mut out = String::new();
    out.push_str(&format!("#{} {}\n", i.id, i.title));
    out.push_str(&format!("  status:  {}\n", i.status.as_str()));
    out.push_str(&format!("  kind:    {}\n", i.kind.as_str()));
    out.push_str(&format!("  priority: {}\n", i.priority));
    out.push_str(&format!(
        "  project: {}\n",
        i.project.as_deref().unwrap_or("?")
    ));
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
    if let Some(b) = &i.body {
        out.push_str(&format!("  body:    {b}\n"));
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

/// 渲染 TSV 表格（表头首行 + tab 分隔数据行，list 默认输出）。
pub fn format_tsv(headers: &[String], rows: &[Vec<String>]) -> String {
    let mut out = String::new();
    out.push_str(&headers.join("\t"));
    out.push('\n');
    for r in rows {
        out.push_str(&r.join("\t"));
        out.push('\n');
    }
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
            priority: 3,
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

    /// format_tsv：表头首行 + tab 分隔数据行，中文原样。
    #[test]
    fn format_tsv_basic() {
        let headers = vec!["ID".to_string(), "Title".to_string()];
        let rows = vec![
            vec!["1".to_string(), "hello".to_string()],
            vec!["2".to_string(), "中文 标题".to_string()],
        ];
        assert_eq!(
            format_tsv(&headers, &rows),
            "ID\tTitle\n1\thello\n2\t中文 标题\n"
        );
    }

    /// format_tsv：空数据行仅输出表头。
    #[test]
    fn format_tsv_empty_rows() {
        assert_eq!(format_tsv(&["A".to_string()], &[]), "A\n");
    }
}
